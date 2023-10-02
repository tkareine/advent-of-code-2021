use std::env;
use std::fs::File;
use std::io::{self, BufRead};

trait CountIncreases {
    fn count_increases(&self) -> usize;
}

impl CountIncreases for [u16] {
    fn count_increases(&self) -> usize {
        self.windows(2).fold(0, |count, xs| {
            let last_x = xs[0];
            let curr_x = xs[1];

            if curr_x > last_x {
                count + 1
            } else {
                count
            }
        })
    }
}

/// CLI usage: cargo run -- input.txt
fn main() {
    let filename = env::args().nth(1).expect("Missing input file");

    let lines: Vec<u16> = io::BufReader::new(File::open(filename).expect("File not found"))
        .lines()
        .map(|l| l.expect("Line not UTF-8").parse().expect("Line not u16"))
        .collect();

    let count_increases_by_groups1 = lines.count_increases();

    println!("count_increases_by_groups1={}", count_increases_by_groups1);

    let count_increases_by_groups3 = {
        let sum_of_groups3: Vec<u16> = lines.windows(3).map(|xs| xs[0] + xs[1] + xs[2]).collect();
        sum_of_groups3.count_increases()
    };

    println!("count_increases_by_groups3={}", count_increases_by_groups3);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty() {
        assert_eq!([].count_increases(), 0);
    }

    #[test]
    fn test_nonempty() {
        assert_eq!([42, 41, 43, 40, 41, 45].count_increases(), 3);
    }
}
