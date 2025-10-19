use nom::bytes::complete::tag;
use nom::bytes::complete::take_while_m_n;
use nom::character::complete::space1;
use nom::sequence::{delimited, separated_pair};
use nom::{Finish, IResult, Parser};
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{self};

type ElementPair = [u8; 2];

type PolymerInsertionRules = HashMap<ElementPair, u8>;

fn parse_polymer(min: usize, max: usize) -> impl for<'a> Fn(&[u8]) -> IResult<&[u8], &[u8]> {
    move |input| take_while_m_n(min, max, |c: u8| c.is_ascii_uppercase()).parse(input)
}

fn parse_polymer_insertion_rule(input: &[u8]) -> IResult<&[u8], (ElementPair, u8)> {
    let (unconsumed, (src, dst)) = separated_pair(
        parse_polymer(2, 2),
        delimited(space1, tag("->"), space1),
        parse_polymer(1, 1),
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
    template: Vec<u8>,
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
                let (incomplete, template) = parse_polymer(2, 100)(line.as_bytes())
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

        let template = polymer_template.ok_or(ReadPolymerError::NoTemplateFound)?;

        if polymer_insertion_rules.is_empty() {
            return Err(ReadPolymerError::NoInsertionRulesFound);
        }

        Ok(Polymer {
            template,
            insertion_rules: polymer_insertion_rules,
        })
    }

    fn histogram_n(&self, n: u8) -> PolymerHistogram {
        let mut histogram = PolymerHistogram::new();
        let mut histogram_cache = PolymerHistogramCache::new();

        for es in self.template.windows(2) {
            let h = rule_histogram_n(
                [es[0], es[1]],
                &self.insertion_rules,
                &mut histogram_cache,
                n,
            );
            histogram.merge(&h);
        }

        if let Some(&last_element) = self.template.last() {
            histogram.add(last_element, 1);
        }

        histogram
    }
}

#[derive(Debug, PartialEq, Eq)]
struct PolymerHistogram(HashMap<u8, u64>);

impl PolymerHistogram {
    fn new() -> PolymerHistogram {
        PolymerHistogram(HashMap::new())
    }

    fn add(&mut self, element: u8, count: u64) {
        self.0
            .entry(element)
            .and_modify(|e| *e += count)
            .or_insert(count);
    }

    fn merge(&mut self, other: &PolymerHistogram) {
        for (element, count) in other.0.iter() {
            self.0
                .entry(*element)
                .and_modify(|e| *e += count)
                .or_insert(*count);
        }
    }

    fn stats(&self) -> Option<PolymerStats> {
        if self.0.is_empty() {
            return None;
        }

        let (most_common_element, most_common_count) =
            self.0.iter().max_by_key(|&(_, count)| count).unwrap();

        let (least_common_element, least_common_count) =
            self.0.iter().min_by_key(|&(_, count)| count).unwrap();

        Some(PolymerStats {
            most_common_element: *most_common_element,
            most_common_count: *most_common_count,
            least_common_element: *least_common_element,
            least_common_count: *least_common_count,
        })
    }
}

impl<const N: usize> From<[(u8, u64); N]> for PolymerHistogram {
    fn from(arr: [(u8, u64); N]) -> Self {
        Self(HashMap::from(arr))
    }
}

struct PolymerHistogramCache(HashMap<(u8, ElementPair), PolymerHistogram>);

impl PolymerHistogramCache {
    const MAX_N: u8 = 40;

    fn new() -> PolymerHistogramCache {
        PolymerHistogramCache(HashMap::new())
    }

    fn get(&self, n: u8, elements: &ElementPair) -> Option<&PolymerHistogram> {
        self.0.get(&(n, *elements))
    }

    fn set(&mut self, n: u8, elements: ElementPair, histogram: PolymerHistogram) {
        self.0.entry((n, elements)).or_insert(histogram);
    }
}

fn rule_histogram_n(
    element_pair: ElementPair,
    insertion_rules: &PolymerInsertionRules,
    histogram_cache: &mut PolymerHistogramCache,
    n: u8,
) -> PolymerHistogram {
    if n == 0 {
        return PolymerHistogram::from([(element_pair[0], 1)]);
    }

    match insertion_rules.get(&element_pair) {
        Some(e) => {
            let mut histogram = PolymerHistogram::new();
            let m = n - 1;

            for el in [[element_pair[0], *e], [*e, element_pair[1]]] {
                if m < PolymerHistogramCache::MAX_N {
                    match histogram_cache.get(m, &el) {
                        Some(cached) => {
                            histogram.merge(cached);
                        }
                        None => {
                            let h = rule_histogram_n(el, insertion_rules, histogram_cache, m);
                            histogram.merge(&h);
                            histogram_cache.set(m, el, h);
                        }
                    };
                } else {
                    let h = rule_histogram_n(el, insertion_rules, histogram_cache, m);
                    histogram.merge(&h);
                }
            }

            histogram
        }
        None => PolymerHistogram::new(),
    }
}

#[derive(Debug, PartialEq, Eq)]
struct PolymerStats {
    most_common_element: u8,
    most_common_count: u64,
    least_common_element: u8,
    least_common_count: u64,
}

impl PolymerStats {
    fn most_and_least_common_element_difference(&self) -> u64 {
        self.most_common_count - self.least_common_count
    }
}

/// CLI usage: cargo run --release -- input.txt
fn main() {
    let filename = env::args().nth(1).expect("Missing input file");

    let polymer = Polymer::read(io::BufReader::new(
        File::open(filename).expect("File not found"),
    ))
    .expect("Failed to read polymer");

    {
        let stats = polymer.histogram_n(10).stats().unwrap();

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
        let stats = polymer.histogram_n(40).stats().unwrap();

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

    const EXAMPLE_INPUT: &str = "NNCB\n\
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
                                 CN -> C";

    #[test]
    fn read_simple_polymer() {
        let polymer = read_polymer(
            "NNCB\n\
            \n\
            CH -> B\n\
            HH -> N",
        );

        assert_eq!(polymer.template, b"NNCB");

        assert_eq!(
            polymer.insertion_rules,
            HashMap::from([(*b"CH", b'B'), (*b"HH", b'N')])
        );
    }

    macro_rules! histogram_n_test {
        ($name:ident : $input:expr , $n:expr , $expected_histogram:expr) => {
            #[test]
            fn $name() {
                let polymer = read_polymer($input);
                let histogram = polymer.histogram_n($n);

                assert_eq!(histogram, $expected_histogram);
            }
        };
    }

    histogram_n_test!(
        histogram_n_0:
        EXAMPLE_INPUT,
        0,
        PolymerHistogram::from([(b'N', 2), (b'C', 1), (b'B', 1)])
    );

    histogram_n_test!(
        histogram_n_1:
        EXAMPLE_INPUT,
        1,
        PolymerHistogram::from([(b'N', 2), (b'C', 2), (b'B', 2), (b'H', 1)])
    );

    histogram_n_test!(
        histogram_n_2:
        EXAMPLE_INPUT,
        2,
        PolymerHistogram::from([(b'N', 2), (b'C', 4), (b'B', 6), (b'H', 1)])
    );

    histogram_n_test!(
        histogram_n_3:
        EXAMPLE_INPUT,
        3,
        PolymerHistogram::from([(b'N', 5), (b'C', 5), (b'B', 11), (b'H', 4)])
    );

    histogram_n_test!(
        histogram_n_4:
        EXAMPLE_INPUT,
        4,
        PolymerHistogram::from([(b'N', 11), (b'C', 10), (b'B', 23), (b'H', 5)])
    );

    #[test]
    fn histogram_stats() {
        let actual_stats = PolymerHistogram::from([(b'N', 5), (b'C', 2), (b'B', 2), (b'H', 1)])
            .stats()
            .unwrap();

        let expected_stats = PolymerStats {
            most_common_element: b'N',
            most_common_count: 5,
            least_common_element: b'H',
            least_common_count: 1,
        };

        assert_eq!(actual_stats, expected_stats);
        assert_eq!(actual_stats.most_and_least_common_element_difference(), 4);
    }

    fn read_polymer(s: &str) -> Polymer {
        Polymer::read(io::BufReader::new(s.as_bytes())).expect("Failed to read polymer")
    }
}
