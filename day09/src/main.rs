use bitvec::prelude as bv;
use std::cmp::Ordering;
use std::collections::{BTreeSet, VecDeque};
use std::env;
use std::fmt;
use std::fs::File;
use std::io::{self, BufRead};
use std::ops::Index;

const MAX_BASIN_HEIGHT: u8 = 9;

const POINT_NEIGHBOURS: [(isize, isize); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Point {
    x: usize,
    y: usize,
}

impl Point {
    fn new(x: usize, y: usize) -> Point {
        Point { x, y }
    }

    fn index1d(self: &Point, max_x: usize) -> usize {
        self.y * (max_x + 1) + self.x
    }

    fn adjacent_points(self: &Point, max: &Point) -> Vec<Point> {
        POINT_NEIGHBOURS
            .iter()
            .flat_map(|(dx, dy)| {
                match (
                    self.x.checked_add_signed(*dx),
                    self.y.checked_add_signed(*dy),
                ) {
                    (Some(x), Some(y)) => {
                        if x <= max.x && y <= max.y {
                            Some(Point { x, y })
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

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

#[derive(Debug, PartialEq, Eq)]
struct HeightPoint {
    height: u8,
    point: Point,
}

impl HeightPoint {
    fn new(height: u8, point: Point) -> HeightPoint {
        HeightPoint { height, point }
    }
}

#[derive(Eq)]
struct LowPoint(HeightPoint);

impl PartialEq for LowPoint {
    fn eq(&self, other: &Self) -> bool {
        self.0.point == other.0.point
    }
}

impl Ord for LowPoint {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.point.cmp(&other.0.point)
    }
}

impl PartialOrd for LowPoint {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for HeightPoint {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}@{}", self.height, self.point)
    }
}

#[derive(Debug)]
enum ParseHeightmapError {
    InvalidLine(String),
    UnexpectedLineLength {
        index: usize,
        expected: usize,
        actual: usize,
    },
}

impl fmt::Display for ParseHeightmapError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use ParseHeightmapError::*;
        match *self {
            InvalidLine(ref line) => write!(f, "Invalid height line: {}", line),
            UnexpectedLineLength {
                index,
                expected,
                actual,
            } => write!(
                f,
                "Unexpected line length at {} (should be {}, but was {})",
                index, expected, actual
            ),
        }
    }
}

#[derive(Debug)]
struct Heightmap {
    data: Vec<Vec<u8>>,
}

impl Heightmap {
    fn max_point(&self) -> Point {
        Point {
            x: self.data[0].len() - 1,
            y: self.data.len() - 1,
        }
    }

    fn collect_low_points(self: &Heightmap) -> Vec<HeightPoint> {
        let mut low_points = BTreeSet::<LowPoint>::new();

        if self.data.is_empty() {
            return vec![];
        }

        let max_point = self.max_point();

        let mut points_visited = bv::bitvec![0; (max_point.x + 1) * (max_point.y + 1)];

        let mut low_point_candidates: VecDeque<Point> = VecDeque::new();

        low_point_candidates.push_back(Point::new(0, 0));

        while let Some(candidate_point) = low_point_candidates.pop_front() {
            let height = self[&candidate_point];

            points_visited.set(candidate_point.index1d(max_point.x), true);

            // println!("C: {}@{}", height, &candidate_point);

            let mut adjacent_points_to_check: VecDeque<Point> = candidate_point
                .adjacent_points(&max_point)
                .into_iter()
                .collect();

            let mut equal_low_points: Vec<Point> = vec![candidate_point];
            let mut maybe_many_equal_low_points = true;

            while let Some(adjacent_point) = adjacent_points_to_check.pop_front() {
                let h = self[&adjacent_point];

                // println!("  A: {}@{}", h, &adjacent_point);

                match h.cmp(&height) {
                    Ordering::Equal => {
                        points_visited.set(adjacent_point.index1d(max_point.x), true);

                        let aps: Vec<Point> = adjacent_point
                            .adjacent_points(&max_point)
                            .into_iter()
                            .filter(|p| {
                                !points_visited[p.index1d(max_point.x)]
                                    && !adjacent_points_to_check.contains(p)
                            })
                            .collect();

                        adjacent_points_to_check.extend(aps);

                        if maybe_many_equal_low_points {
                            equal_low_points.push(adjacent_point);
                        }
                    }
                    Ordering::Less => {
                        equal_low_points.clear();
                        maybe_many_equal_low_points = false;
                        if !points_visited[adjacent_point.index1d(max_point.x)] {
                            low_point_candidates.push_back(adjacent_point);
                        }
                    }
                    Ordering::Greater => {
                        if !points_visited[adjacent_point.index1d(max_point.x)] {
                            low_point_candidates.push_back(adjacent_point);
                        }
                    }
                }
            }

            for p in equal_low_points {
                low_points.insert(LowPoint(HeightPoint::new(self[&p], p)));
            }
        }

        low_points.into_iter().map(|p| p.0).collect()
    }

    fn collect_basin(&self, low_point: &Point) -> Vec<Point> {
        let mut basin_points = Vec::<Point>::new();

        if self.data.is_empty() {
            return basin_points;
        }

        let max_point = self.max_point();

        let mut points_visited = bv::bitvec![0; (max_point.x + 1) * (max_point.y + 1)];

        let mut basin_point_candidates: VecDeque<Point> = VecDeque::new();

        basin_point_candidates.push_back(low_point.clone());

        while let Some(candidate_point) = basin_point_candidates.pop_front() {
            let height = self[&candidate_point];

            points_visited.set(candidate_point.index1d(max_point.x), true);

            if height < MAX_BASIN_HEIGHT {
                // println!("B: {}@{}", height, &candidate_point);

                let cps: Vec<Point> = candidate_point
                    .adjacent_points(&max_point)
                    .into_iter()
                    .filter(|p| {
                        !points_visited[p.index1d(max_point.x)]
                            && !basin_point_candidates.contains(p)
                    })
                    .collect();

                basin_point_candidates.extend(cps);

                basin_points.push(candidate_point);
            }
        }

        basin_points
    }
}

impl Index<&Point> for Heightmap {
    type Output = u8;

    fn index(&self, index: &Point) -> &Self::Output {
        &self.data[index.y][index.x]
    }
}

impl TryFrom<&str> for Heightmap {
    type Error = ParseHeightmapError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let data = value
            .lines()
            .map(|l| parse_height_line(l).ok_or_else(|| ParseHeightmapError::InvalidLine(l.into())))
            .collect::<Result<Vec<Vec<u8>>, ParseHeightmapError>>()?;

        data.try_into()
    }
}

impl TryFrom<Vec<Vec<u8>>> for Heightmap {
    type Error = ParseHeightmapError;

    fn try_from(value: Vec<Vec<u8>>) -> Result<Self, Self::Error> {
        if let Some(err) = check_all_rows_have_same_len(&value) {
            return Err(err);
        }

        Ok(Heightmap { data: value })
    }
}

fn check_all_rows_have_same_len(data: &[Vec<u8>]) -> Option<ParseHeightmapError> {
    let fst_row_len = data[0].len();
    for (idx, row) in data.iter().skip(1).enumerate() {
        if row.len() != fst_row_len {
            return Some(ParseHeightmapError::UnexpectedLineLength {
                index: idx + 1,
                expected: fst_row_len,
                actual: row.len(),
            });
        }
    }
    None
}

fn parse_height_line(line: &str) -> Option<Vec<u8>> {
    line.chars()
        .map(|c| c.to_digit(10).map(|d| d as u8))
        .collect()
}

fn sum_risk_levels(points: &[HeightPoint]) -> u32 {
    points.iter().map(|hp| (hp.height + 1) as u32).sum::<u32>()
}

/// CLI usage: cargo run -- input.txt
fn main() {
    let filename = env::args().nth(1).expect("Missing input file");

    let heightmap: Heightmap = io::BufReader::new(File::open(filename).expect("File not found"))
        .lines()
        .map(|l| {
            let line = l.expect("Line not UTF-8");
            parse_height_line(&line).unwrap_or_else(|| panic!("Invalid height line: {}", line))
        })
        .collect::<Vec<Vec<u8>>>()
        .try_into()
        .unwrap();

    let lps = heightmap.collect_low_points();

    println!("Sum of low point risk levels: {}", sum_risk_levels(&lps));

    let bps_sizes = {
        let mut sizes: Vec<usize> = lps
            .iter()
            .map(|p| heightmap.collect_basin(&p.point).len())
            .collect();
        sizes.sort_by(|a, b| b.cmp(a));
        sizes
    };

    println!(
        "Product of 3 largest basin sizes: {}",
        bps_sizes.iter().take(3).map(|s| *s as u32).product::<u32>()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_low_points_when_one() {
        let map: Heightmap = "339\n\
                              318\n\
                              989\n"
            .try_into()
            .unwrap();

        let lps = map.collect_low_points();

        assert_eq!(lps, vec![HeightPoint::new(1, Point::new(1, 1))]);
    }

    #[test]
    fn collect_low_points_when_many_equal() {
        let map: Heightmap = "339\n\
                              338\n\
                              989\n"
            .try_into()
            .unwrap();

        let lps = map.collect_low_points();

        assert_eq!(
            lps,
            vec![
                HeightPoint::new(3, Point::new(0, 0)),
                HeightPoint::new(3, Point::new(0, 1)),
                HeightPoint::new(3, Point::new(1, 0)),
                HeightPoint::new(3, Point::new(1, 1)),
            ]
        );
    }

    #[test]
    fn collect_low_points_when_9plain() {
        let map: Heightmap = "89123\n\
                              78934\n\
                              89995\n\
                              78989\n"
            .try_into()
            .unwrap();

        let lps = map.collect_low_points();

        assert_eq!(
            lps,
            vec![
                HeightPoint::new(7, Point::new(0, 1)),
                HeightPoint::new(7, Point::new(0, 3)),
                HeightPoint::new(1, Point::new(2, 0)),
                HeightPoint::new(8, Point::new(3, 3)),
            ]
        );
    }

    #[test]
    fn collect_low_points_when_two() {
        let map: Heightmap = "21999\n\
                              39878\n\
                              98567\n\
                              87678\n\
                              98999\n"
            .try_into()
            .unwrap();

        let lps = map.collect_low_points();

        assert_eq!(
            lps,
            vec![
                HeightPoint::new(1, Point::new(1, 0)),
                HeightPoint::new(5, Point::new(2, 2)),
            ]
        );
    }

    #[test]
    fn collect_low_points_when_four() {
        let map: Heightmap = "2199943210\n\
                              3987894921\n\
                              9856789892\n\
                              8767896789\n\
                              9899965678\n"
            .try_into()
            .unwrap();

        let lps = map.collect_low_points();

        assert_eq!(
            lps,
            vec![
                HeightPoint::new(1, Point::new(1, 0)),
                HeightPoint::new(5, Point::new(2, 2)),
                HeightPoint::new(5, Point::new(6, 4)),
                HeightPoint::new(0, Point::new(9, 0)),
            ]
        );
    }

    #[test]
    fn sum_risk_levels_when_four_points() {
        let ps: Vec<HeightPoint> = [1, 5, 5, 0]
            .iter()
            .map(|h| HeightPoint::new(*h, Point::new(0, 0)))
            .collect();

        assert_eq!(sum_risk_levels(&ps), 15);
    }

    #[test]
    fn collect_basin_when_size_3() {
        let map: Heightmap = "219\n\
                              398\n\
                              985\n"
            .try_into()
            .unwrap();

        let bps = map.collect_basin(&Point::new(1, 0));

        assert_eq!(
            bps,
            vec![Point::new(1, 0), Point::new(0, 0), Point::new(0, 1),]
        );
    }

    #[test]
    fn collect_basin_when_size_14() {
        let map: Heightmap = "2199943\n\
                              3987894\n\
                              9856789\n\
                              8767896\n\
                              9899965\n"
            .try_into()
            .unwrap();

        let mut bps = map.collect_basin(&Point::new(2, 2));

        bps.sort();

        assert_eq!(
            bps,
            vec![
                Point::new(0, 3),
                Point::new(1, 2),
                Point::new(1, 3),
                Point::new(1, 4),
                Point::new(2, 1),
                Point::new(2, 2),
                Point::new(2, 3),
                Point::new(3, 1),
                Point::new(3, 2),
                Point::new(3, 3),
                Point::new(4, 1),
                Point::new(4, 2),
                Point::new(4, 3),
                Point::new(5, 2),
            ]
        );
    }
}
