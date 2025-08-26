use std::env;
use std::fmt;
use std::fs::File;
use std::io::{self, BufRead};
use std::num::{IntErrorKind, ParseIntError};
use std::str::FromStr;

#[derive(Debug)]
struct Point {
    x: usize,
    y: usize,
}

#[allow(dead_code)]
#[derive(Debug)]
enum ParsePointError {
    UnexpectedNumComponentsInLine(usize),
    NotComponent {
        parse_error: IntErrorKind,
        component: String,
    },
}

impl FromStr for Point {
    type Err = ParsePointError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let components: Vec<&str> = s.splitn(2, ',').collect();
        if components.len() == 2 {
            let x: usize = components[0].parse().map_err(|e: ParseIntError| {
                ParsePointError::NotComponent {
                    parse_error: *e.kind(),
                    component: components[0].to_string(),
                }
            })?;
            let y: usize = components[1].parse().map_err(|e: ParseIntError| {
                ParsePointError::NotComponent {
                    parse_error: *e.kind(),
                    component: components[1].to_string(),
                }
            })?;
            Ok(Point { x, y })
        } else {
            Err(ParsePointError::UnexpectedNumComponentsInLine(
                components.len(),
            ))
        }
    }
}

#[derive(Debug)]
enum FoldDirection {
    Up,
    Left,
}

#[allow(dead_code)]
#[derive(Debug)]
struct ParseFoldDirectionError(String);

impl FromStr for FoldDirection {
    type Err = ParseFoldDirectionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "y" => Ok(FoldDirection::Up),
            "x" => Ok(FoldDirection::Left),
            _ => Err(ParseFoldDirectionError(s.into())),
        }
    }
}

#[derive(Debug)]
struct FoldInstruction {
    direction: FoldDirection,
    line_position: usize,
}

#[allow(dead_code)]
#[derive(Debug)]
#[allow(clippy::enum_variant_names)]
enum ParseFoldInstructionError {
    UnexpectedNumComponentsInLine(usize),
    UnexpectedDirection(ParseFoldDirectionError),
    UnexpectedLinePosition(IntErrorKind),
}

impl FromStr for FoldInstruction {
    type Err = ParseFoldInstructionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let components: Vec<&str> = s.splitn(2, '=').collect();
        if components.len() == 2 {
            let direction: FoldDirection = components[0]
                .parse()
                .map_err(ParseFoldInstructionError::UnexpectedDirection)?;
            let line_position: usize = components[1].parse().map_err(|e: ParseIntError| {
                ParseFoldInstructionError::UnexpectedLinePosition(*e.kind())
            })?;
            Ok(FoldInstruction {
                direction,
                line_position,
            })
        } else {
            Err(ParseFoldInstructionError::UnexpectedNumComponentsInLine(
                components.len(),
            ))
        }
    }
}

#[derive(Debug)]
struct DotPaper {
    dots: Vec<Vec<bool>>,
    /// In reverse order
    fold_instructions: Vec<FoldInstruction>,
}

impl DotPaper {
    fn count_dots(&self) -> usize {
        self.dots
            .iter()
            .fold(0, |sum, row| sum + row.iter().filter(|d| **d).count())
    }

    fn fold1(&mut self) -> bool {
        match self.fold_instructions.last() {
            Some(FoldInstruction {
                direction,
                line_position,
            }) => {
                let new_dots = match direction {
                    FoldDirection::Up => {
                        let mut new_dots: Vec<Vec<bool>> = self.dots[0..*line_position].into();

                        for (old_row_idx, old_row) in
                            self.dots[(*line_position + 1)..].iter().enumerate()
                        {
                            let new_row_idx = *line_position - 1 - old_row_idx;
                            for (col_idx, has_dot) in old_row.iter().enumerate() {
                                if *has_dot {
                                    new_dots[new_row_idx][col_idx] = true;
                                }
                            }
                        }

                        new_dots
                    }
                    FoldDirection::Left => {
                        let mut new_dots: Vec<Vec<bool>> = Vec::with_capacity(self.dots.len());

                        for old_row in self.dots.iter() {
                            let mut new_row = old_row[0..*line_position].to_vec();
                            for (old_col_idx, has_dot) in
                                old_row[(*line_position + 1)..].iter().enumerate()
                            {
                                let new_col_idx = *line_position - 1 - old_col_idx;
                                if *has_dot {
                                    new_row[new_col_idx] = true;
                                }
                            }
                            new_dots.push(new_row);
                        }

                        new_dots
                    }
                };

                self.dots = new_dots;
                self.fold_instructions.pop();

                true
            }

            None => false,
        }
    }
}

fn max_key_by_key_of_slice<T, B: Ord, F>(slice: &[T], f: F) -> Option<B>
where
    F: Fn(&T) -> B,
{
    let mut found: Option<B> = None;
    for i in slice.iter() {
        let v = f(i);
        match found {
            None => found = Some(v),
            Some(o) if v > o => found = Some(v),
            _ => (),
        };
    }
    found
}

impl From<Vec<DotPaperComponent>> for DotPaper {
    fn from(components: Vec<DotPaperComponent>) -> Self {
        let mut points: Vec<Point> = vec![];
        let mut fold_instructions: Vec<FoldInstruction> = vec![];

        for c in components.into_iter() {
            match c {
                DotPaperComponent::Point(p) => points.push(p),
                DotPaperComponent::FoldInstruction(fi) => fold_instructions.push(fi),
            }
        }

        fold_instructions.reverse();

        let num_cols: usize = max_key_by_key_of_slice(&points, |c| c.x)
            .map(|x| x + 1)
            .unwrap_or(0usize);

        let num_rows: usize = max_key_by_key_of_slice(&points, |c| c.y)
            .map(|y| y + 1)
            .unwrap_or(0usize);

        let empty_row = vec![false; num_cols];

        let mut dots: Vec<Vec<bool>> = vec![empty_row; num_rows];

        points.into_iter().for_each(|c| dots[c.y][c.x] = true);

        DotPaper {
            dots,
            fold_instructions,
        }
    }
}

impl FromStr for DotPaper {
    type Err = ParseDotPaperComponentError;

    fn from_str(str: &str) -> Result<Self, Self::Err> {
        let dot_paper_components = str
            .lines()
            .filter_map(|l| {
                if l.trim().is_empty() {
                    None
                } else {
                    Some(l.parse())
                }
            })
            .collect::<Result<Vec<DotPaperComponent>, ParseDotPaperComponentError>>()?;

        Ok(dot_paper_components.into())
    }
}

impl fmt::Display for DotPaper {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut iter = self.dots.iter().peekable();
        while let Some(row) = iter.next() {
            for col in row.iter() {
                if *col {
                    write!(f, "#")?;
                } else {
                    write!(f, ".")?;
                }
            }
            if iter.peek().is_some() {
                writeln!(f)?;
            }
        }
        Ok(())
    }
}

enum DotPaperComponent {
    Point(Point),
    FoldInstruction(FoldInstruction),
}

#[allow(dead_code)]
#[derive(Debug)]
enum ParseDotPaperComponentError {
    ParsePointError(ParsePointError),
    ParseFoldInstructionError(ParseFoldInstructionError),
}

impl FromStr for DotPaperComponent {
    type Err = ParseDotPaperComponentError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let fold_instruction_key = "fold along ";

        if let Some(s) = s.strip_prefix(fold_instruction_key) {
            let fold_instruction: FoldInstruction = s
                .trim()
                .parse()
                .map_err(ParseDotPaperComponentError::ParseFoldInstructionError)?;
            Ok(DotPaperComponent::FoldInstruction(fold_instruction))
        } else {
            let point: Point = s
                .trim()
                .parse()
                .map_err(ParseDotPaperComponentError::ParsePointError)?;
            Ok(DotPaperComponent::Point(point))
        }
    }
}

/// CLI usage: cargo run --release -- input.txt
fn main() {
    let filename = env::args().nth(1).expect("Missing input file");

    let mut dot_paper: DotPaper = io::BufReader::new(File::open(filename).expect("File not found"))
        .lines()
        .filter_map(|l| {
            let line = l.expect("Line not UTF-8").trim().to_string();
            if line.is_empty() {
                None
            } else {
                Some(line.parse().unwrap_or_else(|e| {
                    panic!("Invalid dot paper component ({:?}) on line: {}", e, line)
                }))
            }
        })
        .collect::<Vec<DotPaperComponent>>()
        .into();

    dot_paper.fold1();

    println!("Num dots after first fold: {}", dot_paper.count_dots());

    while dot_paper.fold1() {}

    println!("Dot paper after all folds:\n{}", dot_paper);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_dot_paper_when_zero_input() {
        let paper: DotPaper = new_zero_input().parse().unwrap();

        assert_eq!(paper.count_dots(), 18);

        assert_eq!(
            paper.to_string(),
            "...#..#..#.\n\
             ....#......\n\
             ...........\n\
             #..........\n\
             ...#....#.#\n\
             ...........\n\
             ...........\n\
             ...........\n\
             ...........\n\
             ...........\n\
             .#....#.##.\n\
             ....#......\n\
             ......#...#\n\
             #..........\n\
             #.#........"
        );
    }

    #[test]
    fn fold_dot_paper_when_zero_input() {
        let mut paper: DotPaper = new_zero_input().parse().unwrap();
        let mut num_folds = 0;
        while paper.fold1() {
            num_folds += 1;
        }

        assert_eq!(num_folds, 2);

        assert_eq!(paper.count_dots(), 16);

        assert_eq!(
            paper.to_string(),
            "#####\n\
             #...#\n\
             #...#\n\
             #...#\n\
             #####\n\
             .....\n\
             ....."
        );
    }

    fn new_zero_input() -> String {
        String::from(
            "6,10\n\
             0,14\n\
             9,10\n\
             0,3\n\
             10,4\n\
             4,11\n\
             6,0\n\
             6,12\n\
             4,1\n\
             0,13\n\
             10,12\n\
             3,4\n\
             3,0\n\
             8,4\n\
             1,10\n\
             2,14\n\
             8,10\n\
             9,0\n\
             \n\
             fold along y=7\n\
             fold along x=5",
        )
    }
}
