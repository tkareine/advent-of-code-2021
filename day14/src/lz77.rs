use std::cmp::min;

pub const MATCH_ENCODING_MARKER: u8 = 0;
pub const MATCH_ENCODING_LEN: usize = 3;
pub const MATCH_MIN_SIZE: usize = 4;
pub const MATCH_MAX_SIZE: usize = 256;

pub fn encode(input: &[u8], output_buf: &mut Vec<u8>) {
    if input.len() < MATCH_MIN_SIZE * 2 {
        output_buf.extend(input);
        return;
    }

    output_buf.extend(&input[0..MATCH_MIN_SIZE]);

    let mut window_idx = 0usize;
    let mut input_idx = MATCH_MIN_SIZE;

    while input_idx + MATCH_MIN_SIZE - 1 < input.len() {
        // println!(
        //     "encode: input_idx={} window_idx={} input.len={}",
        //     input_idx,
        //     window_idx,
        //     input.len()
        // );

        match find_match(input, window_idx, input_idx) {
            Some(ref m) => {
                output_buf.push(MATCH_ENCODING_MARKER);
                m.encode(output_buf);
                input_idx += m.len as usize;
                window_idx = input_idx;
            }
            None => {
                output_buf.push(input[input_idx]);
                input_idx += 1;

                if input_idx - window_idx > MATCH_MAX_SIZE {
                    window_idx += 1;
                }
            }
        }
    }

    if input_idx < input.len() {
        output_buf.extend(&input[input_idx..]);
    }
}

pub fn decode(input: &[u8], output: &mut Vec<u8>) {
    let mut input_idx = 0;

    // println!(
    //     "decode: input.len={} input={} ({:?})",
    //     input.len(),
    //     String::from_utf8_lossy(input),
    //     input,
    // );

    while input_idx < input.len() {
        let c = input[input_idx];

        if c == MATCH_ENCODING_MARKER {
            let Match { offset, len } = Match::decode(&input[(input_idx + 1)..=(input_idx + 2)]);
            let match_start_idx = input_idx - offset as usize;
            let match_end_idx = match_start_idx + len as usize;
            if match_end_idx >= input_idx {
                let output_extend_idx = output.len();
                output.extend(&input[match_start_idx..input_idx]);
                vec_extend_self(output, output_extend_idx, match_end_idx - input_idx);
            } else {
                output.extend(&input[match_start_idx..match_end_idx]);
            }
            input_idx += MATCH_ENCODING_LEN;
        } else {
            output.push(c);
            input_idx += 1;
        }
    }
}

pub struct Match {
    pub offset: u8,
    pub len: u8,
}

impl Match {
    pub fn encode(self: &Match, output: &mut Vec<u8>) {
        output.push(self.offset);
        output.push(self.len);
    }

    pub fn decode(input: &[u8]) -> Match {
        let offset = input[0];
        let len = input[1];
        Match { offset, len }
    }
}

fn find_match(input: &[u8], window_idx: usize, input_idx: usize) -> Option<Match> {
    let pattern = &input[input_idx..(input_idx + MATCH_MIN_SIZE)];

    for match_idx in window_idx..input_idx {
        let mut match_len = MATCH_MIN_SIZE;
        if pattern == &input[match_idx..(match_idx + match_len)] {
            // println!(
            //     "  match match_idx={:?} pattern={} ({:?})",
            //     match_idx,
            //     String::from_utf8_lossy(pattern),
            //     pattern,
            // );
            for _e in 1usize
                ..=min(
                    input.len() - match_len - input_idx,
                    MATCH_MAX_SIZE - MATCH_MIN_SIZE,
                )
            {
                // println!(
                //     "    extend _e={} match_idx={} input_u8={} ({:?}) at {} match_u8={}({:?}) at {}",
                //     _e,
                //     match_idx,
                //     input[input_idx + match_len] as char,
                //     input[input_idx + match_len],
                //     input_idx + match_len,
                //     input[match_idx + match_len] as char,
                //     input[match_idx + match_len],
                //     match_idx + match_len,
                // );
                if input[input_idx + match_len] == input[match_idx + match_len] {
                    match_len += 1;
                } else {
                    break;
                }
            }
            return Some(Match {
                offset: (input_idx - match_idx) as u8,
                len: match_len as u8,
            });
        }
    }

    None
}

fn vec_extend_self(v: &mut Vec<u8>, idx: usize, len: usize) {
    for i in 0..len {
        v.push(v[idx + i]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! encode_test {
        ($name:ident : $input:expr , $expected_encoded:expr) => {
            #[test]
            fn $name() {
                let mut encoded = Vec::new();
                encode($input, &mut encoded);
                assert_eq!(encoded, $expected_encoded);
                let mut decoded = Vec::new();
                decode(&encoded, &mut decoded);
                assert_eq!(decoded, $input);
            }
        };
    }

    encode_test!(encode_no_match_len_0: b"", b"");
    encode_test!(encode_no_match_len_1: b"A", b"A");
    encode_test!(encode_no_match_len_3: b"AAA", b"AAA");
    encode_test!(encode_no_match_len_4: b"AAAA", b"AAAA");
    encode_test!(encode_no_match_len_5: b"AAAAA", b"AAAAA");
    encode_test!(encode_no_match_interleaved: b"ABBABBA", b"ABBABBA");
    encode_test!(encode_one_match_size_4_repeated: b"AAAAAAAA", b"AAAA\0\x04\x04");
    encode_test!(encode_one_match_size_4_at_end: b"CCNBBNANBBN", b"CCNBBNA\0\x05\x04");
    encode_test!(encode_one_match_size_4_at_middle: b"CCNBBNANBBNB", b"CCNBBNA\0\x05\x04B");
    encode_test!(encode_one_match_size_5_at_end: b"CCNBBNAANBBNA", b"CCNBBNAA\0\x06\x05");
    encode_test!(encode_one_match_size_5_at_middle: b"CCNBBNAANBBNAB", b"CCNBBNAA\0\x06\x05B");
    encode_test!(encode_one_match_size_8: b"ABBAZOOMABBAZOOM", b"ABBAZOOM\0\x08\x08");
    encode_test!(encode_one_match_window_progressed: b"ABBAZOOMZOOMABBA", b"ABBAZOOM\0\x04\x04ABBA");
    encode_test!(encode_one_match_len_over_start_pos: b"ANANANANANA", b"ANAN\0\x04\x07");
    encode_test!(encode_two_matches: b"ABBANABBAZOOMZOOM", b"ABBAN\0\x05\x04ZOOM\0\x04\x04");
}
