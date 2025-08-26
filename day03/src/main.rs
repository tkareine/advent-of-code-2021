use bitvec::field::BitField;
use bitvec::prelude as bv;
use std::env;
use std::fs::File;
use std::io::{self, BufRead};

const DIAGNOSTIC_BIT_LEN: usize = 12;

type DiagnosticsBitArray = bv::BitArr!(for DIAGNOSTIC_BIT_LEN);

#[allow(dead_code)]
#[derive(Debug)]
enum Error {
    InvalidDiagnosticLineLength(usize),
    InvalidDiagnosticLineContents(String),
}

fn parse_diagnostics_line(line: &str) -> Result<DiagnosticsBitArray, Error> {
    if line.len() != 12 {
        return Err(Error::InvalidDiagnosticLineLength(line.len()));
    }

    let mut arr: DiagnosticsBitArray = bv::BitArray::ZERO;

    for (i, c) in line.chars().rev().enumerate() {
        let b = match c {
            '0' => Ok(false),
            '1' => Ok(true),
            _ => Err(Error::InvalidDiagnosticLineContents(line.to_owned())),
        }?;

        arr.set(i, b);
    }

    Ok(arr)
}

fn read_gamma_and_epsilon(diagnostics: &[DiagnosticsBitArray]) -> (usize, usize) {
    let mut gamma: DiagnosticsBitArray = bv::BitArray::ZERO;
    let mut epsilon: DiagnosticsBitArray = bv::BitArray::ZERO;

    for i in 0..DIAGNOSTIC_BIT_LEN {
        let num_ones = diagnostics.iter().filter(|d| *d.get(i).unwrap()).count();
        let most_common_bit = num_ones > (diagnostics.len() - num_ones);
        gamma.set(i, most_common_bit);
        epsilon.set(i, !most_common_bit);
    }

    (
        gamma.as_bitslice().load::<usize>(),
        epsilon.as_bitslice().load::<usize>(),
    )
}

fn read_filtered_rating<S>(diagnostics: &[DiagnosticsBitArray], mut select_bit: S) -> usize
where
    S: FnMut(usize, usize) -> bool,
{
    let mut filtered = diagnostics.to_vec();
    let mut safe_idx = Some(DIAGNOSTIC_BIT_LEN - 1);

    while filtered.len() > 1 && safe_idx.is_some() {
        let i = safe_idx.unwrap();

        let num_ones = filtered.iter().filter(|d| *d.get(i).unwrap()).count();
        let selected_bit = select_bit(num_ones, filtered.len());

        filtered.retain(|d| *d.get(i).unwrap() == selected_bit);

        safe_idx = i.checked_sub(1);
    }

    assert!(filtered.len() == 1, "Not found");

    filtered[0].as_bitslice().load::<usize>()
}

/// CLI usage: cargo run -- input.txt
fn main() {
    let filename = env::args().nth(1).expect("Missing input file");

    let diagnostics: Vec<DiagnosticsBitArray> =
        io::BufReader::new(File::open(filename).expect("File not found"))
            .lines()
            .map(|l| {
                parse_diagnostics_line(&l.expect("Line not UTF-8")).expect("Invalid diagnostics")
            })
            .collect();

    let (gamma, epsilon) = read_gamma_and_epsilon(&diagnostics[..]);

    println!("power: {}", gamma * epsilon);

    let oxygen_generator_rating = read_filtered_rating(&diagnostics[..], |num_ones, num_bits| {
        num_ones >= (num_bits - num_ones)
    });

    let co2_scrubber_rating = read_filtered_rating(
        &diagnostics[..],
        #[allow(clippy::overflow_check_conditional)]
        |num_ones, num_bits| num_ones < (num_bits - num_ones),
    );

    println!(
        "life support rating: {}",
        oxygen_generator_rating * co2_scrubber_rating,
    );
}

#[cfg(test)]
mod tests {
    use bitvec::view::BitView;

    use super::*;

    #[test]
    fn parse_diagnostics_line_when_valid_input() {
        let arr = parse_diagnostics_line("110100000101").unwrap();
        assert_eq!(
            arr.as_bitslice(),
            0b1101_0000_0101_usize.view_bits::<bv::Lsb0>()
        );
    }
}
