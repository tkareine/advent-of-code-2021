use std::collections::HashSet;
use std::env;
use std::fmt;
use std::fs::File;
use std::io::{self, BufRead};
use std::result::Result;

#[derive(Debug)]
enum ParseSignalsError {
    Patterns(usize),
    Outputs(usize),
    Tokens(usize, usize),
}

impl fmt::Display for ParseSignalsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use ParseSignalsError::*;
        match *self {
            Patterns(n) => write!(f, "Invalid number of signal patterns ({})", n),
            Outputs(n) => write!(f, "Invalid number of signal outputs ({})", n),
            Tokens(pn, on) => write!(
                f,
                "Invalid number of signal patterns ({}) and outputs ({})",
                pn, on
            ),
        }
    }
}

const NUM_SIGNAL_PATTERNS: usize = 10;
const NUM_SIGNAL_OUTPUTS: usize = 4;

#[derive(Debug)]
struct Signals<'a> {
    patterns: [&'a str; NUM_SIGNAL_PATTERNS],
    outputs: [&'a str; NUM_SIGNAL_OUTPUTS],
}

impl<'a> Signals<'a> {
    fn decipher(self: &Signals<'a>) -> Option<u16> {
        SignalPatterns::parse_patterns(self.patterns)?.parse_outputs(self.outputs)
    }

    fn parse(line: &str) -> Result<Signals, ParseSignalsError> {
        let mut patterns: Vec<&str> = vec![];
        let mut outputs: Vec<&str> = vec![];
        let mut read_outputs = false;

        for token in line.split_ascii_whitespace() {
            if token == "|" {
                if !read_outputs {
                    read_outputs = true;
                    continue;
                } else {
                    break;
                }
            }

            if read_outputs {
                outputs.push(token);
            } else {
                patterns.push(token);
            }
        }

        match (patterns.len(), outputs.len()) {
            (NUM_SIGNAL_PATTERNS, NUM_SIGNAL_OUTPUTS) => Ok(Signals {
                patterns: patterns.try_into().unwrap(),
                outputs: outputs.try_into().unwrap(),
            }),
            (NUM_SIGNAL_PATTERNS, n) => Err(ParseSignalsError::Outputs(n)),
            (n, NUM_SIGNAL_OUTPUTS) => Err(ParseSignalsError::Patterns(n)),
            (pn, on) => Err(ParseSignalsError::Tokens(pn, on)),
        }
    }
}

#[derive(Debug)]
struct SignalPatterns {
    chars_of_0: HashSet<char>,
    chars_of_2: HashSet<char>,
    chars_of_3: HashSet<char>,
    chars_of_5: HashSet<char>,
    chars_of_6: HashSet<char>,
    chars_of_9: HashSet<char>,
}

impl SignalPatterns {
    /// The algorithm to decipher patterns of output digits:
    ///
    /// Syntax:
    ///
    /// Loop \<n\>:
    /// \<digit\>: \<rule to decipher\>
    ///
    /// Loop 1:
    ///   1: has 2 chars
    ///   7: has 3 chars
    ///   4: has 4 chars
    ///   8: has 7 chars
    ///
    /// Loop 2:
    ///   9: has 6 chars && has all chars of 4
    ///   0: has 6 chars && has all chars of 1
    ///   3: has 5 chars && has all chars of 1
    ///   6: has 6 chars
    ///
    /// Loop 3:
    ///   5: has 5 chars && difference to the pattern of 9 leaves 0 chars
    ///   2: has 5 chars && difference to the pattern of 9 leaves 1 char
    fn parse_patterns(patterns: [&str; NUM_SIGNAL_PATTERNS]) -> Option<SignalPatterns> {
        let mut opt_chars_of_1: Option<HashSet<char>> = None;
        let mut opt_chars_of_4: Option<HashSet<char>> = None;

        for pat in patterns {
            match pat.len() {
                2 => {
                    opt_chars_of_1 = Some(pat.chars().collect());
                }
                4 => {
                    opt_chars_of_4 = Some(pat.chars().collect());
                }
                _ => {}
            }
        }

        let chars_of_1 = opt_chars_of_1?;
        let chars_of_4 = opt_chars_of_4?;

        let mut opt_chars_of_0: Option<HashSet<char>> = None;
        let mut opt_chars_of_3: Option<HashSet<char>> = None;
        let mut opt_chars_of_9: Option<HashSet<char>> = None;
        let mut opt_chars_of_6: Option<HashSet<char>> = None;

        for pat in patterns {
            match pat.len() {
                5 => {
                    let cs: HashSet<char> = pat.chars().collect();
                    if cs.intersection(&chars_of_1).count() == 2 {
                        opt_chars_of_3 = Some(cs);
                    }
                }
                6 => {
                    let cs: HashSet<char> = pat.chars().collect();
                    if cs.intersection(&chars_of_4).count() == 4 {
                        opt_chars_of_9 = Some(cs);
                    } else if cs.intersection(&chars_of_1).count() == 2 {
                        opt_chars_of_0 = Some(cs);
                    } else {
                        opt_chars_of_6 = Some(cs);
                    }
                }
                _ => {}
            }
        }

        let chars_of_3 = opt_chars_of_3?;
        let chars_of_9 = opt_chars_of_9?;
        let chars_of_0 = opt_chars_of_0?;
        let chars_of_6 = opt_chars_of_6?;

        let mut opt_chars_of_5: Option<HashSet<char>> = None;
        let mut opt_chars_of_2: Option<HashSet<char>> = None;

        for pat in patterns.iter().filter(|p| p.len() == 5) {
            let cs: HashSet<char> = pat.chars().collect();
            if cs == chars_of_3 {
                continue; // handled already
            }
            match cs.difference(&chars_of_9).count() {
                0 => {
                    opt_chars_of_5 = Some(cs);
                }
                1 => {
                    opt_chars_of_2 = Some(cs);
                }
                _ => {}
            }
        }

        let chars_of_5 = opt_chars_of_5?;
        let chars_of_2 = opt_chars_of_2?;

        Some(SignalPatterns {
            chars_of_0,
            chars_of_2,
            chars_of_3,
            chars_of_5,
            chars_of_6,
            chars_of_9,
        })
    }

    fn parse_outputs(self: &SignalPatterns, outputs: [&str; NUM_SIGNAL_OUTPUTS]) -> Option<u16> {
        let n3 = self.output_token_to_digit(outputs[0])?;
        let n2 = self.output_token_to_digit(outputs[1])?;
        let n1 = self.output_token_to_digit(outputs[2])?;
        let n0 = self.output_token_to_digit(outputs[3])?;
        Some(n0 + 10 * n1 + 100 * n2 + 1000 * n3)
    }

    fn output_token_to_digit(self: &SignalPatterns, token: &str) -> Option<u16> {
        match token.len() {
            2 => Some(1),
            3 => Some(7),
            4 => Some(4),
            7 => Some(8),
            _ => {
                let cs: HashSet<char> = token.chars().collect();
                if cs == self.chars_of_0 {
                    Some(0)
                } else if cs == self.chars_of_2 {
                    Some(2)
                } else if cs == self.chars_of_3 {
                    Some(3)
                } else if cs == self.chars_of_5 {
                    Some(5)
                } else if cs == self.chars_of_6 {
                    Some(6)
                } else if cs == self.chars_of_9 {
                    Some(9)
                } else {
                    None
                }
            }
        }
    }
}

fn count_digits(digits: &[u8], mut n: u16) -> u16 {
    let mut count = 0;
    while n > 0 {
        let x = (n % 10) as u8;
        if digits.iter().any(|d| x == *d) {
            count += 1;
        }
        n /= 10;
    }
    count
}

/// CLI usage: cargo run -- input.txt
fn main() {
    let filename = env::args().nth(1).expect("Missing input file");

    let signal_outputs: Vec<u16> =
        io::BufReader::new(File::open(filename).expect("File not found"))
            .lines()
            .map(|l| {
                let line = l.expect("Line not UTF-8");
                let signals = Signals::parse(&line)
                    .unwrap_or_else(|err| panic!("Invalid signal output line: {}", err));
                signals
                    .decipher()
                    .unwrap_or_else(|| panic!("Unrecognized signal pattern: {}", line))
            })
            .collect();

    {
        let digits = vec![1, 4, 7, 8];
        println!(
            "num digits {:?}: {}",
            &digits,
            signal_outputs
                .iter()
                .map(|n| count_digits(&digits, *n))
                .sum::<u16>()
        );
    }

    println!(
        "sum: {}",
        signal_outputs.iter().map(|o| *o as u32).sum::<u32>()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decipher() {
        let line = "acedgfb cdfbe gcdfa fbcad dab cefabd cdfgeb eafb cagedb ab |
cdfeb fcadb cdfeb cdbaf";
        let s = Signals::parse(line).unwrap();
        assert_eq!(s.decipher(), Some(5353));
    }
}
