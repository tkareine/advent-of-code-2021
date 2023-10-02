use std::env;
use std::fs::File;
use std::io::{self, BufRead};
use std::num::ParseIntError;

fn parse_fish_state_line(s: &str) -> Result<Vec<u8>, ParseIntError> {
    s.split(',').map(|n| n.parse::<u8>()).collect()
}

const NEW_FISH_TIMER: u8 = 8;
const FISH_RESET_TIMER: u8 = 6;

#[derive(Debug, Clone)]
struct FishSwarm {
    pub num_fishes_by_timer: [u64; NEW_FISH_TIMER as usize + 1],
}

impl FishSwarm {
    fn new(fish_timers: &Vec<u8>) -> FishSwarm {
        let mut arr = [0; NEW_FISH_TIMER as usize + 1];

        for &fish_timer in fish_timers {
            arr[fish_timer as usize] += 1;
        }

        FishSwarm {
            num_fishes_by_timer: arr,
        }
    }

    fn simulate_fish_spawns_in_day(self: &mut FishSwarm) {
        let mut arr = [0; NEW_FISH_TIMER as usize + 1];

        for (timer, &num_fishes) in self.num_fishes_by_timer.iter().enumerate() {
            if timer == 0 {
                arr[NEW_FISH_TIMER as usize] = num_fishes;
                arr[FISH_RESET_TIMER as usize] = num_fishes;
            } else {
                arr[timer - 1] += num_fishes;
            }
        }

        self.num_fishes_by_timer = arr;
    }

    fn simulate_fish_spawns(self: &mut FishSwarm, num_days: u32) {
        for _ in 0..num_days {
            self.simulate_fish_spawns_in_day();
        }
    }

    fn sum_fishes(self: &FishSwarm) -> u64 {
        self.num_fishes_by_timer.iter().sum()
    }
}

/// CLI usage: cargo run -- input.txt
fn main() {
    let filename = env::args().nth(1).expect("Missing input file");

    let swarm: FishSwarm = {
        let timers: Vec<u8> = io::BufReader::new(File::open(filename).expect("File not found"))
            .lines()
            .next()
            .expect("File is empty")
            .map(|l| {
                parse_fish_state_line(&l)
                    .unwrap_or_else(|err| panic!("Invalid fish state line: {}", err))
            })
            .expect("Line not UTF-8");

        FishSwarm::new(&timers)
    };

    for days in [80, 256] {
        let mut s = swarm.clone();
        s.simulate_fish_spawns(days);
        println!("Number of fishes after {} days: {}", days, s.sum_fishes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_it() {
        let mut swarm = FishSwarm::new(&vec![3, 4, 3, 1, 2]);
        swarm.simulate_fish_spawns_in_day();
        assert_eq!(swarm.num_fishes_by_timer, [1, 1, 2, 1, 0, 0, 0, 0, 0]);
        swarm.simulate_fish_spawns_in_day();
        assert_eq!(swarm.num_fishes_by_timer, [1, 2, 1, 0, 0, 0, 1, 0, 1]);
    }
}
