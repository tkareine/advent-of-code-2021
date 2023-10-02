use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::{i32, space1};
use nom::combinator::value;
use nom::sequence::separated_pair;
use nom::{Finish, IResult};
use std::env;
use std::fs::File;
use std::io::{self, BufRead};

#[derive(Clone, Debug, PartialEq)]
enum Direction {
    Up,
    Down,
    Forward,
}

#[derive(Debug, PartialEq)]
struct Movement {
    dx: i32,
    dy: i32,
}

fn parse_direction(input: &str) -> IResult<&str, Direction> {
    use Direction::*;
    alt((
        value(Up, tag("up")),
        value(Down, tag("down")),
        value(Forward, tag("forward")),
    ))(input)
}

fn parse_movement(input: &str) -> IResult<&str, Movement> {
    use Direction::*;
    let (unconsumed, (direction, delta)) = separated_pair(parse_direction, space1, i32)(input)?;
    let movement = match direction {
        Up => Movement { dx: 0, dy: -delta },
        Down => Movement { dx: 0, dy: delta },
        Forward => Movement { dx: delta, dy: 0 },
    };
    Ok((unconsumed, movement))
}

/// CLI usage: cargo run -- input.txt
fn main() {
    let filename = env::args().nth(1).expect("Missing input file");

    let movements: Vec<Movement> =
        io::BufReader::new(File::open(filename).expect("File not found"))
            .lines()
            .map(|l| {
                parse_movement(&l.expect("Line not UTF-8"))
                    .finish()
                    .expect("Unknown movement")
                    .1
            })
            .collect();

    let pos_direct = movements.iter().fold((0, 0), |(pos_x, pos_y), mov| {
        (pos_x + mov.dx, pos_y + mov.dy)
    });

    println!("pos_direct (x * y): {}", pos_direct.0 * pos_direct.1);

    let pos_aimed = movements
        .iter()
        .fold((0, 0, 0), |(pos_x, pos_y, aim), mov| {
            let pos_x_new = pos_x + mov.dx;
            let pos_y_new = pos_y + aim * mov.dx;
            let aim_new = aim + mov.dy;
            (pos_x_new, pos_y_new, aim_new)
        });

    println!("pos_aimed (x * y): {}", pos_aimed.0 * pos_aimed.1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_movement_down() {
        let (_, m) = parse_movement("down 42").unwrap();
        assert_eq!(m, Movement { dx: 0, dy: 42 });
    }
}
