use std::env;
use std::fs::File;
use std::io::{self, BufRead};

#[derive(Debug, PartialEq)]
enum ChunksError {
    Illegal {
        closing_chunk: &'static Chunk,
    },
    Incomplete {
        missing_closing_chunks: Vec<&'static Chunk>,
    },
    Invalid {
        char: char,
    },
}

#[derive(Debug, PartialEq)]
struct Chunk {
    open_char: char,
    close_char: char,
    illegal_close_char_score: u16,
    complete_close_char_score: u8,
}

const CHUNKS: [Chunk; 4] = [
    Chunk {
        open_char: '(',
        close_char: ')',
        illegal_close_char_score: 3,
        complete_close_char_score: 1,
    },
    Chunk {
        open_char: '[',
        close_char: ']',
        illegal_close_char_score: 57,
        complete_close_char_score: 2,
    },
    Chunk {
        open_char: '{',
        close_char: '}',
        illegal_close_char_score: 1197,
        complete_close_char_score: 3,
    },
    Chunk {
        open_char: '<',
        close_char: '>',
        illegal_close_char_score: 25137,
        complete_close_char_score: 4,
    },
];

impl Chunk {
    fn get(c: char) -> Option<&'static Chunk> {
        match c {
            '(' | ')' => Some(&CHUNKS[0]),
            '[' | ']' => Some(&CHUNKS[1]),
            '{' | '}' => Some(&CHUNKS[2]),
            '<' | '>' => Some(&CHUNKS[3]),
            _ => None,
        }
    }
}

fn check_chunks_error(str: &str) -> Option<ChunksError> {
    use ChunksError::*;

    let mut stack: Vec<&Chunk> = vec![];

    for c in str.chars() {
        if let Some(chunk) = Chunk::get(c) {
            if chunk.open_char == c {
                stack.push(chunk);
            } else if let Some(expected_chunk) = stack.pop() {
                if expected_chunk != chunk {
                    return Some(Illegal {
                        closing_chunk: chunk,
                    });
                }
            } else {
                return Some(Illegal {
                    closing_chunk: chunk,
                });
            }
        } else {
            return Some(Invalid { char: c });
        }
    }

    if stack.is_empty() {
        None
    } else {
        stack.reverse();
        Some(Incomplete {
            missing_closing_chunks: stack,
        })
    }
}

fn middle_score_of_missing_closing_chunkses(chunkses: Vec<Vec<&Chunk>>) -> Option<u64> {
    fn line_score(chunks: &[&Chunk]) -> u64 {
        chunks.iter().fold(0u64, |sum, c| {
            sum * 5 + (c.complete_close_char_score as u64)
        })
    }

    if chunkses.is_empty() {
        return None;
    }

    let mut scores: Vec<u64> = chunkses.iter().map(|cs| line_score(cs)).collect();
    scores.sort();
    Some(scores[scores.len() / 2])
}

/// CLI usage: cargo run -- input.txt
fn main() {
    let filename = env::args().nth(1).expect("Missing input file");

    let (illegal_closing_chunks, missing_closing_chunkses): (Vec<&Chunk>, Vec<Vec<&Chunk>>) = {
        let mut illegal_closing_chunks: Vec<&Chunk> = vec![];
        let mut missing_closing_chunkses: Vec<Vec<&Chunk>> = vec![];

        for line in io::BufReader::new(File::open(filename).expect("File not found")).lines() {
            match check_chunks_error(&line.expect("Line not UTF-8")) {
                Some(ChunksError::Illegal { closing_chunk }) => {
                    illegal_closing_chunks.push(closing_chunk)
                }
                Some(ChunksError::Incomplete {
                    missing_closing_chunks,
                }) => missing_closing_chunkses.push(missing_closing_chunks),
                _ => (),
            }
        }

        (illegal_closing_chunks, missing_closing_chunkses)
    };

    println!(
        "Sum of illegal closing chars: {}",
        illegal_closing_chunks
            .iter()
            .map(|c| c.illegal_close_char_score as u32)
            .sum::<u32>()
    );

    println!(
        "Middle score of completing missing closing chars: {}",
        middle_score_of_missing_closing_chunkses(missing_closing_chunkses).unwrap()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_chunks_error_when_none() {
        assert!(check_chunks_error("(()[{<>}][])").is_none());
    }

    #[test]
    fn check_chunks_error_invalid() {
        assert_eq!(
            check_chunks_error("a"),
            Some(ChunksError::Invalid { char: 'a' })
        );
    }

    #[test]
    fn check_chunks_error_incomplete() {
        assert_eq!(
            check_chunks_error("([][<"),
            Some(ChunksError::Incomplete {
                missing_closing_chunks: vec!['>', ']', ')']
                    .into_iter()
                    .map(|c| Chunk::get(c).unwrap())
                    .collect()
            })
        );
    }

    #[test]
    fn check_chunks_error_illegal() {
        assert_eq!(
            check_chunks_error("([<])"),
            Some(ChunksError::Illegal {
                closing_chunk: Chunk::get(']').unwrap()
            })
        );
    }
}
