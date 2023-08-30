use nom::bytes::complete::tag;
use nom::character::complete::{space1, u32};
use nom::sequence::{delimited, separated_pair};
use nom::{Finish, IResult};
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{self, BufRead};

type Point = (u32, u32);

#[derive(Debug, PartialEq)]
struct Line {
    begin: Point,
    end: Point,
}

impl Line {
    fn new(begin: Point, end: Point) -> Line {
        Line { begin, end }
    }

    fn is_horizontal(self: &Line) -> bool {
        self.begin.1 == self.end.1
    }

    fn is_vertical(self: &Line) -> bool {
        self.begin.0 == self.end.0
    }

    fn is_diagonal_45deg(self: &Line) -> bool {
        let dx = self.end.0 as i32 - self.begin.0 as i32;
        let dy = self.end.1 as i32 - self.begin.1 as i32;
        dx.abs() == dy.abs()
    }

    fn points(self: &Line) -> Vec<Point> {
        if !(self.is_horizontal() || self.is_vertical() || self.is_diagonal_45deg()) {
            panic!("Unsupported line angle: {:?}", self);
        }

        let ddx = (self.end.0 as i32 - self.begin.0 as i32).signum();
        let ddy = (self.end.1 as i32 - self.begin.1 as i32).signum();

        let mut p = self.begin;
        let mut points = Vec::new();

        loop {
            points.push(p);

            if p == self.end {
                break;
            }

            p.0 = (p.0 as i32 + ddx) as u32;
            p.1 = (p.1 as i32 + ddy) as u32;
        }

        points
    }
}

fn parse_point(input: &str) -> IResult<&str, Point> {
    separated_pair(u32, tag(","), u32)(input)
}

fn parse_line(input: &str) -> IResult<&str, Line> {
    let (unconsumed, (begin, end)) = separated_pair(
        parse_point,
        delimited(space1, tag("->"), space1),
        parse_point,
    )(input)?;
    let line = Line::new(begin, end);
    Ok((unconsumed, line))
}

#[derive(Debug)]
struct Space {
    points: HashMap<Point, u32>,
}

impl Space {
    fn new() -> Space {
        Space {
            points: HashMap::new(),
        }
    }

    fn draw_line(self: &mut Space, line: &Line) {
        for p in line.points() {
            let overlaps = self.points.entry(p).or_insert(0);
            *overlaps += 1;
        }
    }

    fn count_points_with_overlaps(self: &Space, min_overlap: u32) -> usize {
        self.points
            .iter()
            .filter(|(_, v)| **v >= min_overlap)
            .count()
    }
}

/// CLI usage: cargo run -- input.txt
fn main() {
    let filename = env::args().nth(1).expect("Missing input file");

    let lines: Vec<Line> = io::BufReader::new(File::open(filename).expect("File not found"))
        .lines()
        .map(|l| {
            parse_line(&l.expect("Line not UTF-8"))
                .finish()
                .expect("Unknown line")
                .1
        })
        .collect();

    let num_points_from_hv_lines_with_min_2_overlaps = {
        let mut space = Space::new();
        for l in lines
            .iter()
            .filter(|l| l.is_horizontal() || l.is_vertical())
        {
            space.draw_line(l);
        }
        space.count_points_with_overlaps(2)
    };

    println!(
        "Num points from horizontal/vertical lines with min. 2 overlaps: {}",
        num_points_from_hv_lines_with_min_2_overlaps
    );

    let num_points_from_hvd_lines_with_min_2_overlaps = {
        let mut space = Space::new();
        for l in lines
            .iter()
            .filter(|l| l.is_horizontal() || l.is_vertical() || l.is_diagonal_45deg())
        {
            space.draw_line(l);
        }
        space.count_points_with_overlaps(2)
    };

    println!(
        "Num points from horizontal/vertical/diagonal lines with min. 2 overlaps: {}",
        num_points_from_hvd_lines_with_min_2_overlaps
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_on_dot_line() {
        let l = Line::new((2, 5), (2, 5));
        assert!(l.is_horizontal());
        assert!(l.is_vertical());
        assert!(l.is_diagonal_45deg());
        assert_eq!(l.points(), vec![(2, 5)]);
    }

    #[test]
    fn points_on_horizontal_line() {
        let l = Line::new((5, 2), (2, 2));
        assert!(l.is_horizontal());
        assert!(!l.is_vertical());
        assert!(!l.is_diagonal_45deg());
        assert_eq!(l.points(), vec![(5, 2), (4, 2), (3, 2), (2, 2)]);
    }

    #[test]
    fn points_on_vertical_line() {
        let l = Line::new((2, 5), (2, 2));
        assert!(!l.is_horizontal());
        assert!(l.is_vertical());
        assert!(!l.is_diagonal_45deg());
        assert_eq!(l.points(), vec![(2, 5), (2, 4), (2, 3), (2, 2)]);
    }

    #[test]
    fn points_on_diagonal_45deg_line() {
        let l = Line::new((9, 4), (4, 9));
        assert!(!l.is_horizontal());
        assert!(!l.is_vertical());
        assert!(l.is_diagonal_45deg());
        assert_eq!(
            l.points(),
            vec![(9, 4), (8, 5), (7, 6), (6, 7), (5, 8), (4, 9)]
        );
    }
}
