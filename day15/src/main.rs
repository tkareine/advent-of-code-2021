use std::collections::{BinaryHeap, HashMap};
use std::env;
use std::fs::File;
use std::io::{self, BufRead};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Point {
    x: usize,
    y: usize,
}

#[allow(dead_code)]
#[derive(Debug)]
enum ReadCaveError {
    InvalidLine(String),
    EmptyInput,
    InconsistentRowSize { row_idx: usize },
}

#[derive(Debug, PartialEq)]
struct Cave {
    risk_levels: Vec<Vec<u8>>,
}

impl Cave {
    fn read(reader: impl BufRead) -> Result<Cave, ReadCaveError> {
        let mut risk_levels = Vec::new();

        for (line_num, line) in reader.lines().enumerate() {
            let line = line
                .map_err(|e| ReadCaveError::InvalidLine(e.to_string()))?
                .trim()
                .to_owned();

            if line.is_empty() {
                continue;
            }

            risk_levels.push(parse_risk_levels(line_num, &line)?);
        }

        let first_row_len = risk_levels.first().ok_or(ReadCaveError::EmptyInput)?.len();

        if let Some((row_idx, _)) = risk_levels
            .iter()
            .enumerate()
            .find(|(_, row)| row.len() != first_row_len)
        {
            return Err(ReadCaveError::InconsistentRowSize { row_idx });
        }

        Ok(Cave { risk_levels })
    }

    fn max_y(&self) -> usize {
        self.risk_levels.len() - 1
    }

    fn max_x(&self) -> usize {
        self.risk_levels[0].len() - 1
    }

    fn repeat(&self, num_times: usize) -> Cave {
        assert!(num_times > 0, "num_times must be positive");

        let num_src_rows = self.max_y() + 1;
        let num_src_cols = self.max_x() + 1;

        let src_rs = &self.risk_levels;
        let mut dst_rs = Vec::with_capacity(num_times * num_src_rows);

        for y in 0..num_times * num_src_rows {
            let tile_y = (y / num_src_rows) as u16;
            let src_row = &src_rs[y % num_src_rows];
            let mut dst_row = Vec::with_capacity(num_times * num_src_cols);

            for x in 0..num_times * num_src_cols {
                let tile_x = (x / num_src_cols) as u16;
                let src_level = src_row[x % num_src_cols];
                let dst_level = saturate_risk_level(tile_y + tile_x + src_level as u16);
                dst_row.push(dst_level);
            }

            dst_rs.push(dst_row);
        }

        Cave {
            risk_levels: dst_rs,
        }
    }

    /// Dijkstra's algorithm for finding shortest path from `start_point` to
    /// `end_point`.
    ///
    /// Sources:
    ///
    /// * [Wikipedia - Dijkstra's algorithm](https://en.wikipedia.org/wiki/Dijkstra's_algorithm)
    /// * [Rust stdlib - binary_heap](https://doc.rust-lang.org/std/collections/binary_heap/index.html)
    fn shortest_path(&self, start_point: &Point, end_point: &Point) -> Option<ShortestPathResult> {
        // Current shortest distances from `start_point` to a `Point`
        let mut distances_from_start = HashMap::<Point, u64>::new();

        // Positions to consider next in priority order
        let mut heap = BinaryHeap::<Pos>::new();

        // Current paths with shortest distances to `to_point` (key) from
        // `from_point` (value). Use to find the reverse path from `end_point`
        // to `start_point`.
        let mut prev_points = HashMap::<Point, Point>::new();

        distances_from_start.insert(start_point.clone(), 0);

        heap.push(Pos {
            distance: 0,
            point: start_point.clone(),
        });

        while let Some(Pos {
            distance,
            point: from_point,
        }) = heap.pop()
        {
            // println!("sp> distance={} from_point={:?}", distance, &from_point);

            if from_point == *end_point {
                return Some(ShortestPathResult {
                    distance,
                    path: make_path(end_point, prev_points),
                });
            }

            if let Some(best_distance) = distances_from_start.get(&from_point)
                && distance > *best_distance
            {
                // println!("sp> …discard as obsolete");
                continue;
            }

            for to_point in self.neighbours(&from_point) {
                let point_risk = self.risk_levels[to_point.y][to_point.x];
                let new_distance = distance + point_risk as u64;

                let found_shorter_path = match distances_from_start.get(&to_point) {
                    Some(&best_distance) => new_distance < best_distance,
                    None => true,
                };

                if found_shorter_path {
                    // println!(
                    //     "sp> …add as candidate: new_distance={} to_point={:?}",
                    //     new_distance, to_point
                    // );
                    heap.push(Pos {
                        distance: new_distance,
                        point: to_point.clone(),
                    });
                    distances_from_start.insert(to_point.clone(), new_distance);
                    prev_points.insert(to_point.clone(), from_point.clone());
                }
            }
        }

        None
    }

    fn neighbours(&self, point: &Point) -> Vec<Point> {
        let mut ps = Vec::new();

        for (dx, dy) in [(0i8, -1i8), (1, 0), (0, 1), (-1, 0)] {
            let new_y = point.y as i128 + dy as i128;
            let new_x = point.x as i128 + dx as i128;

            if new_y >= 0
                && (new_y as usize) < self.risk_levels.len()
                && new_x >= 0
                && (new_x as usize) < self.risk_levels[new_y as usize].len()
            {
                ps.push(Point {
                    x: new_x as usize,
                    y: new_y as usize,
                });
            }
        }

        ps
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Pos {
    distance: u64,
    point: Point,
}

impl Ord for Pos {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other
            .distance
            .cmp(&self.distance)
            .then_with(|| self.point.cmp(&other.point))
    }
}

impl PartialOrd for Pos {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, PartialEq)]
struct ShortestPathResult {
    distance: u64,
    path: Vec<Point>,
}

fn make_path(end_point: &Point, prev_points: HashMap<Point, Point>) -> Vec<Point> {
    let mut path = Vec::new();

    let mut curr_point = end_point;

    while let Some(prev_point) = prev_points.get(curr_point) {
        path.push(prev_point.clone());
        curr_point = prev_point;
    }

    // Last element is always the start point
    path.pop();

    path.reverse();

    path
}

fn parse_risk_levels(line_number: usize, line: &str) -> Result<Vec<u8>, ReadCaveError> {
    let mut risk_levels = Vec::with_capacity(line.len());

    for (i, c) in line.bytes().enumerate() {
        if c.is_ascii_digit() {
            risk_levels.push(c - b'0');
        } else {
            return Err(ReadCaveError::InvalidLine(format!(
                r#"Invalid character at position {} on line {}: expected ASCII digit, got "{}""#,
                i, line_number, c as char
            )));
        }
    }

    Ok(risk_levels)
}

fn saturate_risk_level(level: u16) -> u8 {
    if level == 0 {
        0
    } else {
        ((level - 1) % 9) as u8 + 1
    }
}

/// CLI usage: cargo run --release -- input.txt
fn main() {
    let filename = env::args().nth(1).expect("Missing input file");

    let cave = Cave::read(io::BufReader::new(
        File::open(filename).expect("File not found"),
    ))
    .expect("Failed to read cave");

    let start_point = Point { x: 0, y: 0 };

    {
        let sp = cave
            .shortest_path(
                &start_point,
                &Point {
                    x: cave.max_x(),
                    y: cave.max_y(),
                },
            )
            .expect("No shortest path found for the original cave");

        println!(
            "Shortest path distance of the original cave: {}",
            sp.distance
        );
    }

    {
        let cave_repeated = cave.repeat(5);

        let sp = cave_repeated
            .shortest_path(
                &start_point,
                &Point {
                    x: cave_repeated.max_x(),
                    y: cave_repeated.max_y(),
                },
            )
            .expect("No shortest path found for the repeated cave");

        println!(
            "Shortest path distance of the repeated cave: {}",
            sp.distance
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE_INPUT: &str = "1234\n\
                                1208\n\
                                9012";

    #[test]
    fn read_simple_cave() {
        let actual_cave = read_cave(SIMPLE_INPUT);

        let expected_cave = Cave {
            risk_levels: vec![vec![1, 2, 3, 4], vec![1, 2, 0, 8], vec![9, 0, 1, 2]],
        };

        assert_eq!(actual_cave, expected_cave);
    }

    macro_rules! neighbours_test {
        ($name:ident : $cave_input:expr , $point:expr , $expected_neighbours:expr) => {
            #[test]
            fn $name() {
                let cave = read_cave($cave_input);
                let actual_ns = cave.neighbours($point);

                assert_eq!(actual_ns, $expected_neighbours);
            }
        };
    }

    neighbours_test!(
        neighbours_0_0:
        SIMPLE_INPUT,
        &Point { x: 0, y: 0 },
        vec![Point {x: 1, y: 0}, Point {x: 0, y: 1}]
    );

    neighbours_test!(
       neighbours_1_1:
       SIMPLE_INPUT,
       &Point { x: 1, y: 1 },
       vec![Point {x: 1, y: 0}, Point {x: 2, y: 1}, Point {x: 1, y: 2}, Point {x: 0, y: 1}]
    );

    neighbours_test!(
       neighbours_3_2:
       SIMPLE_INPUT,
       &Point { x: 3, y: 2 },
       vec![Point {x: 3, y: 1}, Point {x: 2, y: 2}]
    );

    macro_rules! shortest_path_test {
        ($name:ident : $cave_input:expr , $expected_shortest_path:expr) => {
            #[test]
            fn $name() {
                let cave = read_cave($cave_input);
                let actual_sp = cave.shortest_path(
                    &Point { x: 0, y: 0 },
                    &Point {
                        x: cave.max_x(),
                        y: cave.max_y(),
                    },
                );

                assert_eq!(actual_sp, $expected_shortest_path);
            }
        };
    }

    shortest_path_test!(
        shortest_path_when_found_in_trivial_cave:
        "11",
        Some(ShortestPathResult { distance: 1, path: vec![] })
    );

    shortest_path_test!(
        shortest_path_when_found_in_simple_cave:
        SIMPLE_INPUT,
        Some(
            ShortestPathResult {
                distance: 6,
                path: vec![
                    Point { x: 0, y: 1 },
                    Point { x: 1, y: 1 },
                    Point { x: 2, y: 1 },
                    Point { x: 2, y: 2 },
                ],
            }
        )
    );

    #[test]
    fn sortest_path_when_not_found() {
        let cave = read_cave(SIMPLE_INPUT);
        let actual_sp = cave.shortest_path(
            &Point { x: 0, y: 0 },
            &Point {
                x: cave.max_x(),
                y: cave.max_y() + 1,
            },
        );

        assert_eq!(actual_sp, None);
    }

    #[test]
    fn saturate_risk_levels() {
        assert_eq!(saturate_risk_level(0), 0);
        assert_eq!(saturate_risk_level(1), 1);
        assert_eq!(saturate_risk_level(8), 8);
        assert_eq!(saturate_risk_level(9), 9);
        assert_eq!(saturate_risk_level(10), 1);
        assert_eq!(saturate_risk_level(11), 2);
        assert_eq!(saturate_risk_level(17), 8);
        assert_eq!(saturate_risk_level(18), 9);
        assert_eq!(saturate_risk_level(19), 1);
    }

    #[test]
    fn repeat_cave() {
        let actual_cave = read_cave(SIMPLE_INPUT).repeat(2);

        let expected_cave = Cave {
            risk_levels: vec![
                vec![1, 2, 3, 4, 2, 3, 4, 5],
                vec![1, 2, 0, 8, 2, 3, 1, 9],
                vec![9, 0, 1, 2, 1, 1, 2, 3],
                vec![2, 3, 4, 5, 3, 4, 5, 6],
                vec![2, 3, 1, 9, 3, 4, 2, 1],
                vec![1, 1, 2, 3, 2, 2, 3, 4],
            ],
        };

        assert_eq!(actual_cave, expected_cave);
    }

    fn read_cave(s: &str) -> Cave {
        Cave::read(io::BufReader::new(s.as_bytes())).expect("Failed to read cave")
    }
}
