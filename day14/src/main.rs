use nom::bytes::complete::tag;
use nom::bytes::complete::take_while_m_n;
use nom::character::complete::space1;
use nom::sequence::{delimited, separated_pair};
use nom::{Finish, IResult, Parser};
use std::cmp::min;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{self};
use std::time::Instant;

use crate::lz77::Match;

mod lz77;

const DECODING_CHUNK_MAX_LEN: usize = 8192;

type PolymerInsertionRules = HashMap<[u8; 2], u8>;

fn parse_polymer(min: usize) -> impl for<'a> Fn(&[u8]) -> IResult<&[u8], &[u8]> {
    move |input| take_while_m_n(min, 50, |c: u8| c.is_ascii_uppercase()).parse(input)
}

fn parse_polymer_insertion_rule(input: &[u8]) -> IResult<&[u8], ([u8; 2], u8)> {
    let (unconsumed, (src, dst)) = separated_pair(
        parse_polymer(2),
        delimited(space1, tag("->"), space1),
        parse_polymer(1),
    )
    .parse(input)?;

    Ok((unconsumed, ([src[0], src[1]], dst[0])))
}

#[allow(dead_code)]
#[derive(Debug)]
enum ReadPolymerError {
    InvalidLine(String),
    NoTemplateFound,
    NoInsertionRulesFound,
    InvalidPolymerTemplate(String),
    InvalidPolymerRule(String),
}

struct Polymer {
    /// LZ77 encoded, guaranteed to be non-empty.
    template_encoded: Vec<u8>,
    /// Guaranteed to be non-empty.
    insertion_rules: PolymerInsertionRules,
}

impl Polymer {
    fn read(reader: impl io::BufRead) -> Result<Polymer, ReadPolymerError> {
        let mut polymer_template: Option<Vec<u8>> = None;
        let mut polymer_insertion_rules: PolymerInsertionRules = HashMap::new();

        for line in reader.lines() {
            let line = line
                .map_err(|e| ReadPolymerError::InvalidLine(e.to_string()))?
                .trim()
                .to_owned();

            if line.is_empty() {
                continue;
            }

            if polymer_template.is_none() {
                let (incomplete, template) = parse_polymer(2)(line.as_bytes())
                    .finish()
                    .map_err(|_| ReadPolymerError::InvalidPolymerTemplate(line.to_owned()))?;

                if !incomplete.is_empty() {
                    return Err(ReadPolymerError::InvalidPolymerTemplate(line));
                }

                polymer_template = Some(template.to_vec());
            } else {
                let (src, dst) = parse_polymer_insertion_rule(line.as_bytes())
                    .finish()
                    .map_err(|_| ReadPolymerError::InvalidPolymerRule(line.to_owned()))?
                    .1;
                polymer_insertion_rules.insert(src, dst);
            }
        }

        let template = polymer_template
            .map(|t| {
                let mut encoded = Vec::new();
                lz77::encode(&t, &mut encoded);
                encoded
            })
            .ok_or(ReadPolymerError::NoTemplateFound)?;

        if polymer_insertion_rules.is_empty() {
            return Err(ReadPolymerError::NoInsertionRulesFound);
        }

        Ok(Polymer {
            template_encoded: template,
            insertion_rules: polymer_insertion_rules,
        })
    }

    fn count_elements(&self) -> HashMap<u8, usize> {
        let mut counts_by_element = HashMap::new();

        let mut decoded_buf: Vec<u8> = Vec::with_capacity(2 * DECODING_CHUNK_MAX_LEN);

        let mut template_encoded_idx = 0;

        while template_encoded_idx < self.template_encoded.len() {
            decoded_buf.truncate(0);

            let decoding_chunk_len = get_safe_decoding_chunk_len(
                &self.template_encoded[template_encoded_idx..],
                DECODING_CHUNK_MAX_LEN,
            );

            if template_encoded_idx + decoding_chunk_len > self.template_encoded.len() {
                println!(">> {:?}", &self.template_encoded[template_encoded_idx..]);
            }

            lz77::decode(
                &self.template_encoded
                    [template_encoded_idx..(template_encoded_idx + decoding_chunk_len)],
                &mut decoded_buf,
            );

            for element in decoded_buf.iter() {
                *counts_by_element.entry(*element).or_insert(0) += 1;
            }

            template_encoded_idx += decoding_chunk_len;
        }

        counts_by_element
    }

    fn stats(&self) -> Option<PolymerStats> {
        let element_counts = self.count_elements();

        if element_counts.is_empty() {
            return None;
        }

        let (most_common_element, most_common_count) = element_counts
            .iter()
            .max_by_key(|&(_, count)| count)
            .unwrap();

        let (least_common_element, least_common_count) = element_counts
            .iter()
            .min_by_key(|&(_, count)| count)
            .unwrap();

        Some(PolymerStats {
            most_common_element: *most_common_element,
            most_common_count: *most_common_count,
            least_common_element: *least_common_element,
            least_common_count: *least_common_count,
        })
    }

    fn step1(&mut self) {
        let mut new_template: Vec<u8> = Vec::new();
        let mut decoded_buf: Vec<u8> = Vec::with_capacity(2 * DECODING_CHUNK_MAX_LEN);
        let mut encoded_buf: Vec<u8> = Vec::with_capacity(2 * DECODING_CHUNK_MAX_LEN);
        let mut grow_buf: Vec<u8> = Vec::with_capacity(4 * DECODING_CHUNK_MAX_LEN);

        let mut previous_chunk_last_element: Option<u8> = None;

        let mut template_encoded_idx = 0;

        while template_encoded_idx < self.template_encoded.len() {
            decoded_buf.truncate(0);
            encoded_buf.truncate(0);
            grow_buf.truncate(0);

            let decoding_chunk_len = get_safe_decoding_chunk_len(
                &self.template_encoded[template_encoded_idx..],
                DECODING_CHUNK_MAX_LEN,
            );

            // println!(
            //     "step1 decoding_chunk_len={} template_encoded.len={} template_encoded={} ({:?})",
            //     decoding_chunk_len,
            //     self.template_encoded.len(),
            //     String::from_utf8_lossy(&self.template_encoded),
            //     &self.template_encoded
            // );

            lz77::decode(
                &self.template_encoded
                    [template_encoded_idx..(template_encoded_idx + decoding_chunk_len)],
                &mut decoded_buf,
            );

            // println!(
            //     "  decoding_chunk_len={} decoded_buf.len={} decoded_buf={} ({:?})",
            //     decoding_chunk_len,
            //     decoded_buf.len(),
            //     String::from_utf8_lossy(&decoded_buf),
            //     &decoded_buf
            // );

            grow_polymer_template(
                &self.insertion_rules,
                &decoded_buf,
                previous_chunk_last_element,
                &mut grow_buf,
            );

            // println!(
            //     "  grow_buf.len={} grow_buf={} ({:?})",
            //     grow_buf.len(),
            //     String::from_utf8_lossy(&grow_buf),
            //     &grow_buf
            // );

            lz77::encode(&grow_buf, &mut encoded_buf);

            // println!(
            //     "  encoded_buf.len={} encoded_buf={} ({:?})",
            //     encoded_buf.len(),
            //     String::from_utf8_lossy(&encoded_buf),
            //     &encoded_buf
            // );

            new_template.extend(&encoded_buf);

            previous_chunk_last_element = decoded_buf.last().copied();
            template_encoded_idx += decoding_chunk_len;
        }

        self.template_encoded = new_template;
    }
}

fn get_safe_decoding_chunk_len(encoded_buf: &[u8], decoding_chunk_max_len: usize) -> usize {
    if encoded_buf.len() <= decoding_chunk_max_len {
        return encoded_buf.len();
    }

    let last_window_idx = decoding_chunk_max_len.saturating_sub(lz77::MATCH_MAX_SIZE);

    let max_last_window_len = {
        let available_len = decoding_chunk_max_len - last_window_idx;
        min(available_len, lz77::MATCH_MAX_SIZE)
    };

    let last_window_slice = &encoded_buf[last_window_idx..(last_window_idx + max_last_window_len)];

    // Find in the last possible window, but excluding non-complete match
    // encodings
    let adjusted_last_window_len = match last_window_slice
        [..(last_window_slice.len() - lz77::MATCH_ENCODING_LEN + 1)]
        .iter()
        .position(|c| *c == lz77::MATCH_ENCODING_MARKER)
    {
        Some(pos) => {
            let Match { offset, len } =
                lz77::Match::decode(&last_window_slice[pos..(pos + lz77::MATCH_ENCODING_LEN)]);

            match pos.checked_sub(offset as usize) {
                Some(match_start_pos) => {
                    let match_end_pos = match_start_pos + len as usize;
                    if match_end_pos <= max_last_window_len {
                        pos + lz77::MATCH_ENCODING_LEN
                    } else {
                        match_start_pos
                    }
                }
                None => pos + lz77::MATCH_ENCODING_LEN,
            }
        }
        None => 0,
    };

    last_window_idx + adjusted_last_window_len
}

fn grow_polymer_template(
    insertion_rules: &PolymerInsertionRules,
    template: &[u8],
    previous_chunk_last_element: Option<u8>,
    output_buf: &mut Vec<u8>,
) {
    let mut template_idx = 0usize;
    let mut last_element = previous_chunk_last_element;

    while template_idx < template.len() {
        let curr_element = template[template_idx];

        match (last_element, curr_element) {
            (Some(le), ce) => {
                let src = [le, ce];
                match insertion_rules.get(&src) {
                    Some(insertion) => {
                        output_buf.push(*insertion);
                        output_buf.push(curr_element);
                    }
                    None => {
                        output_buf.push(curr_element);
                    }
                }
            }
            (None, ce) => {
                output_buf.push(ce);
            }
        }

        template_idx += 1;
        last_element = Some(curr_element);
    }
}

struct PolymerStats {
    most_common_element: u8,
    most_common_count: usize,
    least_common_element: u8,
    least_common_count: usize,
}

impl PolymerStats {
    fn most_and_least_common_element_difference(&self) -> usize {
        self.most_common_count - self.least_common_count
    }
}

/// CLI usage: cargo run --release -- input.txt
fn main() {
    let filename = env::args().nth(1).expect("Missing input file");
    let mut polymer = Polymer::read(io::BufReader::new(
        File::open(filename).expect("File not found"),
    ))
    .expect("Failed to read polymer");

    {
        for i in 0..10 {
            let start = Instant::now();

            polymer.step1();

            println!(
                "step={} template.len={} time={:?}",
                i,
                polymer.template_encoded.len(),
                start.elapsed()
            )
        }

        let stats = polymer.stats().unwrap();

        println!(
            "After 10 steps:\n\
             - most common element: {} ({})\n\
             - least common element: {} ({})\n\
             - their count difference: {}",
            stats.most_common_element,
            stats.most_common_count,
            stats.least_common_element,
            stats.least_common_count,
            stats.most_and_least_common_element_difference()
        );
    }

    {
        for i in 10..40 {
            let start = Instant::now();

            polymer.step1();

            println!(
                "step={} template.len={} time={:?}",
                i,
                polymer.template_encoded.len(),
                start.elapsed()
            );
        }

        let stats = polymer.stats().unwrap();

        println!(
            "After 40 steps:\n\
             - most common element: {} ({})\n\
             - least common element: {} ({})\n\
             - their count difference: {}",
            stats.most_common_element,
            stats.most_common_count,
            stats.least_common_element,
            stats.least_common_count,
            stats.most_and_least_common_element_difference()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_simple_polymer() {
        let polymer = read_polymer(
            "NNCB\n\
            \n\
            CH -> B\n\
            HH -> N",
        );

        assert_eq!(polymer.template_encoded, b"NNCB");

        assert_eq!(
            polymer.insertion_rules,
            HashMap::from([(*b"CH", b'B'), (*b"HH", b'N')])
        );
    }

    #[test]
    fn grow_polymer_template_once() {
        let polymer = read_polymer(
            "AABC\n\
            \n\
            AA -> B\n\
            AB -> C\n\
            BC -> A",
        );

        let mut buf = Vec::<u8>::new();

        grow_polymer_template(
            &polymer.insertion_rules,
            &polymer.template_encoded,
            None,
            &mut buf,
        );

        assert_eq!(buf, b"ABACBAC");
    }

    macro_rules! get_safe_decoding_chunk_len_test {
        ($name:ident : $input:expr , $decoding_chunk_max_len:expr , $expected_decoding_chunk_len:expr) => {
            #[test]
            fn $name() {
                assert_eq!(
                    get_safe_decoding_chunk_len($input, $decoding_chunk_max_len),
                    $expected_decoding_chunk_len
                );
            }
        };
    }

    get_safe_decoding_chunk_len_test!(get_safe_decoding_chunk_len_no_match: b"PNFCVHKKOC", 32, 10);
    get_safe_decoding_chunk_len_test!(get_safe_decoding_chunk_len_match:  b"AABAABAABAABAABAABAABAABAABSO\0\x02\x04NAANAAN", 32, 32);

    #[test]
    fn step_polymer() {
        let mut polymer = read_polymer(
            "NNCB\n\
             \n\
             CH -> B\n\
             HH -> N\n\
             CB -> H\n\
             NH -> C\n\
             HB -> C\n\
             HC -> B\n\
             HN -> C\n\
             NN -> C\n\
             BH -> H\n\
             NC -> B\n\
             NB -> B\n\
             BN -> B\n\
             BB -> N\n\
             BC -> B\n\
             CC -> N\n\
             CN -> C",
        );

        polymer.step1();

        assert_eq!(polymer.template_encoded, b"NCNBCHB");

        polymer.step1();

        assert_eq!(polymer.template_encoded, b"NBCCNBBBCBHCB");

        polymer.step1();

        assert_eq!(polymer.template_encoded, b"NBBBCNCCNBBNBNBBCHBHH\0\x06\x04");

        polymer.step1();

        assert_eq!(
            polymer.template_encoded,
            b"NBBNBNBBCCNBCN\0\x06\x04BNB\0\x03\x04BNB\0\x03\x04CBHCBHHNHCBB\0\x0c\x05"
        );
    }

    fn read_polymer(s: &str) -> Polymer {
        Polymer::read(io::BufReader::new(s.as_bytes())).expect("Failed to read polymer")
    }
}
