use std::collections::BTreeMap;
use std::env;
use std::fs::File;
use std::io::{self, BufRead};
use std::num::ParseIntError;

fn parse_positions_line(s: &str) -> Result<BTreeMap<u16, u32>, ParseIntError> {
    let poses = s.split(',').map(|n| n.parse::<u16>());

    let mut num_by_pos = BTreeMap::new();

    for pos in poses {
        let num = num_by_pos.entry(pos?).or_insert(0);
        *num += 1;
    }

    Ok(num_by_pos)
}

fn find_min_cost_position<F>(num_by_pos: &BTreeMap<u16, u32>, cost_fn: F) -> Option<(u16, u64)>
where
    F: Fn(u32) -> u64,
{
    if num_by_pos.is_empty() {
        return None;
    }

    let min_pos = 0;
    let max_pos = *num_by_pos.last_key_value().unwrap().0;

    let mut min_cost_found: Option<(u16, u64)> = None;

    for dst_pos in min_pos..=max_pos {
        let mut cost: u64 = 0;

        for (&src_pos, &num) in num_by_pos {
            let pos_delta = ((dst_pos as i32) - (src_pos as i32)).unsigned_abs();

            cost += (num as u64) * cost_fn(pos_delta);

            if let Some((_, cost_found)) = min_cost_found {
                if cost > cost_found {
                    break;
                }
            }
        }

        match min_cost_found {
            Some((_, cost_found)) => {
                if cost < cost_found {
                    min_cost_found = Some((dst_pos, cost));
                }
            }
            None => {
                min_cost_found = Some((dst_pos, cost));
            }
        }
    }

    min_cost_found
}

/// CLI usage: cargo run -- input.txt
fn main() {
    let filename = env::args().nth(1).expect("missing input file");

    let num_by_pos: BTreeMap<u16, u32> =
        io::BufReader::new(File::open(filename).expect("File not found"))
            .lines()
            .next()
            .expect("File is empty")
            .map(|l| {
                parse_positions_line(&l)
                    .unwrap_or_else(|err| panic!("Invalid positions line: {}", err))
            })
            .expect("Line not UTF-8");

    let cost_fns: [(&str, &dyn Fn(u32) -> u64); 2] = [
        ("constant", &|d| d as u64),
        ("increasing", &|d| {
            let mut sum = 0;
            for s in 1..=d {
                sum += s;
            }
            sum as u64
        }),
    ];

    for (cost_fn_desc, cost_fn) in cost_fns {
        let (pos, cost) = find_min_cost_position(&num_by_pos, cost_fn).unwrap();

        println!(
            "min cost position when {} cost fn: pos={}, cost={}",
            cost_fn_desc, pos, cost
        );
    }
}
