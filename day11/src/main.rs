use bitvec::prelude as bv;
use std::env;
use std::fs::File;
use std::io::{self, BufRead};
use std::ops::{Index, IndexMut};
use std::str::FromStr;

#[allow(dead_code)]
#[derive(Debug)]
enum ParseOctopusMapError {
    EnergyLevel(char),
    UnexpectedNumCols(usize),
    UnexpectedNumRows(usize),
}

const OCTOPUS_MAP_COLS: usize = 10;
const OCTOPUS_MAP_ROWS: usize = 10;

type OctopusLines = [[u8; OCTOPUS_MAP_COLS]; OCTOPUS_MAP_ROWS];

const OCTOPUS_MIN_FLASH_ENERGY_LEVEL: u8 = 10;

const OCTOPUS_NEIGHBOUR_DELTAS: [(isize, isize); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
];

#[derive(Debug)]
struct OctopusLine([u8; OCTOPUS_MAP_COLS]);

impl FromStr for OctopusLine {
    type Err = ParseOctopusMapError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut res = [0; OCTOPUS_MAP_COLS];
        let mut num_cols = 0;

        for (idx, c) in s.chars().take(OCTOPUS_MAP_COLS).enumerate() {
            if let Some(n) = c.to_digit(10) {
                res[idx] = n as u8;
            } else {
                return Err(ParseOctopusMapError::EnergyLevel(c));
            }
            num_cols += 1;
        }

        if num_cols != OCTOPUS_MAP_COLS {
            return Err(ParseOctopusMapError::UnexpectedNumCols(num_cols));
        }

        Ok(OctopusLine(res))
    }
}

#[derive(Debug)]
struct XY(usize, usize);

impl XY {
    fn index1d(&self) -> usize {
        let XY(x, y) = *self;
        y * OCTOPUS_MAP_ROWS + x
    }

    fn neighbours(&self) -> Vec<XY> {
        OCTOPUS_NEIGHBOUR_DELTAS
            .iter()
            .filter_map(|(dx, dy)| {
                let XY(x, y) = self;
                let mx = x.checked_add_signed(*dx);
                let my = y.checked_add_signed(*dy);
                match (mx, my) {
                    (Some(x), Some(y)) => {
                        if y < OCTOPUS_MAP_ROWS && x < OCTOPUS_MAP_COLS {
                            Some(XY(x, y))
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            })
            .collect()
    }
}

#[derive(Debug, PartialEq, Eq)]
struct OctopusMap {
    rows: OctopusLines,
}

impl OctopusMap {
    fn new(rows: OctopusLines) -> OctopusMap {
        OctopusMap { rows }
    }

    /// Run one step of energy simulation, returning the number of
    /// flashes happened during the step.
    fn step_energy_simulation(&mut self) -> u8 {
        let mut num_flashes = 0u8;
        let mut have_flashed = bv::bitvec![0; OCTOPUS_MAP_COLS * OCTOPUS_MAP_ROWS];
        let mut about_to_flash: Vec<XY> = vec![];

        for (y, row) in self.rows.iter_mut().enumerate() {
            for (x, energy_level) in row.iter_mut().enumerate() {
                *energy_level += 1;
                if *energy_level >= OCTOPUS_MIN_FLASH_ENERGY_LEVEL {
                    about_to_flash.push(XY(x, y));
                }
            }
        }

        while let Some(xy) = about_to_flash.pop() {
            if have_flashed[xy.index1d()] {
                continue;
            }

            self[&xy] = 0;

            num_flashes += 1;

            have_flashed.set(xy.index1d(), true);

            for n_xy in xy.neighbours() {
                if !have_flashed[n_xy.index1d()] {
                    let energy_level = &mut self[&n_xy];
                    *energy_level += 1;
                    if *energy_level >= OCTOPUS_MIN_FLASH_ENERGY_LEVEL {
                        about_to_flash.push(n_xy);
                    }
                }
            }
        }

        num_flashes
    }
}

impl Index<&XY> for OctopusMap {
    type Output = u8;

    fn index(&self, index: &XY) -> &Self::Output {
        let XY(x, y) = *index;
        &self.rows[y][x]
    }
}

impl IndexMut<&XY> for OctopusMap {
    fn index_mut(&mut self, index: &XY) -> &mut Self::Output {
        let XY(x, y) = *index;
        &mut self.rows[y][x]
    }
}

impl FromStr for OctopusMap {
    type Err = ParseOctopusMapError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let lines = s
            .lines()
            .take(OCTOPUS_MAP_ROWS)
            .map(|r| r.parse())
            .collect::<Result<Vec<OctopusLine>, ParseOctopusMapError>>()?;

        lines.try_into()
    }
}

impl TryFrom<Vec<OctopusLine>> for OctopusMap {
    type Error = ParseOctopusMapError;

    fn try_from(value: Vec<OctopusLine>) -> Result<Self, Self::Error> {
        let data: OctopusLines = value
            .into_iter()
            .map(|l| l.0)
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|v: Vec<[u8; OCTOPUS_MAP_COLS]>| {
                ParseOctopusMapError::UnexpectedNumRows(v.len())
            })?;

        Ok(OctopusMap::new(data))
    }
}

/// CLI usage: cargo run -- input.txt
fn main() {
    let filename = env::args().nth(1).expect("Missing input file");

    let mut map: OctopusMap = io::BufReader::new(File::open(filename).expect("File not found"))
        .lines()
        .take(OCTOPUS_MAP_ROWS)
        .map(|l| {
            let line = &l.expect("Line not UTF-8");
            line.parse()
                .unwrap_or_else(|e| panic!("Invalid octopus map line ({:?}): {}", e, line))
        })
        .collect::<Vec<OctopusLine>>()
        .try_into()
        .unwrap();

    let mut sum_flashes_after_100_steps: usize = 0;
    let mut num_steps: usize = 0;
    let mut all_octopuses_flash_at_step: Option<usize> = None;

    while all_octopuses_flash_at_step.is_none() || num_steps < 100 {
        num_steps += 1;

        let num_flashes = map.step_energy_simulation() as usize;

        if all_octopuses_flash_at_step.is_none()
            && num_flashes == OCTOPUS_MAP_COLS * OCTOPUS_MAP_ROWS
        {
            all_octopuses_flash_at_step = Some(num_steps);
        }

        if num_steps < 100 {
            sum_flashes_after_100_steps += num_flashes;
        }
    }

    println!(
        "Sum flashes after 100 steps: {}\n\
         All octopuses flash at step {}",
        sum_flashes_after_100_steps,
        all_octopuses_flash_at_step.unwrap()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_simulation() {
        let mut map: OctopusMap = "5483143223\n\
                                   2745854711\n\
                                   5264556173\n\
                                   6141336146\n\
                                   6357385478\n\
                                   4167524645\n\
                                   2176841721\n\
                                   6882881134\n\
                                   4846848554\n\
                                   5283751526"
            .parse()
            .unwrap();

        let mut num_flashes = map.step_energy_simulation();

        assert_eq!(num_flashes, 0);

        let mut expected_map: OctopusMap = "6594254334\n\
                                            3856965822\n\
                                            6375667284\n\
                                            7252447257\n\
                                            7468496589\n\
                                            5278635756\n\
                                            3287952832\n\
                                            7993992245\n\
                                            5957959665\n\
                                            6394862637"
            .parse()
            .unwrap();

        assert_eq!(map, expected_map);

        num_flashes = map.step_energy_simulation();

        assert_eq!(num_flashes, 35);

        expected_map = "8807476555\n\
                        5089087054\n\
                        8597889608\n\
                        8485769600\n\
                        8700908800\n\
                        6600088989\n\
                        6800005943\n\
                        0000007456\n\
                        9000000876\n\
                        8700006848"
            .parse()
            .unwrap();

        assert_eq!(map, expected_map);
    }
}
