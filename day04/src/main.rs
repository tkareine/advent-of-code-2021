use std::env;
use std::fs::File;
use std::io::{self, BufRead};
use std::mem::take;
use std::ops::{Index, IndexMut};

fn parse_draws(line: &str) -> Vec<u8> {
    line.split(',')
        .map(|s| s.trim().parse::<u8>().expect("invalid number to draw"))
        .collect()
}

const BINGO_COLS: usize = 5;
const BINGO_ROWS: usize = 5;

type BingoLine = [Option<u8>; BINGO_COLS];
type BingoLines = [BingoLine; BINGO_ROWS];

#[derive(Debug)]
struct BingoBoard {
    rows: BingoLines,
}

impl BingoBoard {
    fn new(rows: BingoLines) -> BingoBoard {
        BingoBoard { rows }
    }

    /// Check if a number drawn appears on the board, marking the
    /// matching number(s) and returning `true` if so. Otherwise returns
    /// `false`.
    fn mark_draw(&mut self, draw: u8) -> bool {
        let mut draw_hit = false;

        for line in &mut self.rows {
            for x in line {
                if let Some(n) = *x {
                    if n == draw {
                        *x = None;
                        draw_hit = true;
                    }
                }
            }
        }

        draw_hit
    }

    fn numbers(&self) -> BingoNumbersIter {
        BingoNumbersIter {
            current_row: 0,
            current_col: 0,
            board: self,
        }
    }

    fn numbers_sum(&self) -> u32 {
        self.numbers().flatten().map(|n| n as u32).sum()
    }

    fn has_bingo(&self) -> bool {
        self.has_bingo_by_horizontal_line() || self.has_bingo_by_vertical_line()
    }

    fn has_bingo_by_horizontal_line(&self) -> bool {
        self.rows.iter().any(|r| r.iter().all(|c| c.is_none()))
    }

    fn has_bingo_by_vertical_line(&self) -> bool {
        for x in 0..BINGO_COLS {
            for y in 0..BINGO_ROWS {
                match self[y][x] {
                    Some(_) => {
                        break;
                    }
                    None => {
                        if y == BINGO_ROWS - 1 {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }
}

impl Index<usize> for BingoBoard {
    type Output = BingoLine;

    fn index(&self, index: usize) -> &Self::Output {
        &self.rows[index]
    }
}

impl IndexMut<usize> for BingoBoard {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.rows[index]
    }
}

struct BingoNumbersIter<'a> {
    current_row: usize,
    current_col: usize,
    board: &'a BingoBoard,
}

impl<'a> Iterator for BingoNumbersIter<'a> {
    type Item = Option<u8>;

    fn next(&mut self) -> Option<Option<u8>> {
        if self.current_row < BINGO_ROWS {
            let n = self.board[self.current_row][self.current_col];
            self.current_col += 1;
            if self.current_col >= BINGO_COLS {
                self.current_col = 0;
                self.current_row += 1;
            }
            Some(n)
        } else {
            None
        }
    }
}

fn parse_bingo_line(line: &str) -> BingoLine {
    let mut res: BingoLine = [None; BINGO_COLS];
    let mut num_nums = 0;

    for (idx, c) in line.split_ascii_whitespace().take(BINGO_COLS).enumerate() {
        let n = c.parse::<u8>().ok();
        if n.is_none() {
            panic!("invalid number as bingo input: {}", c)
        }
        res[idx] = n;
        num_nums += 1;
    }

    assert!(num_nums == BINGO_COLS);

    res
}

fn parse_bingo_board(lines: &[&str]) -> BingoBoard {
    assert!(lines.len() == BINGO_ROWS);

    let mut res: BingoLines = [[None; BINGO_COLS]; BINGO_ROWS];

    for (idx, r) in lines.iter().enumerate() {
        res[idx] = parse_bingo_line(r);
    }

    BingoBoard::new(res)
}

fn parse_bingo_boards(lines: &[&str]) -> Vec<BingoBoard> {
    lines.chunks(BINGO_ROWS).map(parse_bingo_board).collect()
}

type FirstAndLastBingoBoards = (Option<(u8, BingoBoard)>, Option<(u8, BingoBoard)>);

fn draw_first_and_last_bingo(draws: Vec<u8>, bbs: Vec<BingoBoard>) -> FirstAndLastBingoBoards {
    let mut obbs: Vec<Option<BingoBoard>> = bbs.into_iter().map(Some).collect();

    let mut fst_bingo: Option<(u8, BingoBoard)> = None;

    for n in draws {
        for idx in 0..obbs.len() {
            let obb = &mut obbs[idx];
            if let Some(bb) = obb {
                bb.mark_draw(n);
                if bb.has_bingo() {
                    let found_bingo = take(obb).unwrap();
                    match fst_bingo {
                        Some(_) => {
                            if obbs.iter().flatten().count() == 0 {
                                return (fst_bingo, Some((n, found_bingo)));
                            }
                        }
                        None => {
                            fst_bingo = Some((n, found_bingo));
                        }
                    }
                }
            }
        }
    }

    (fst_bingo, None)
}

/// CLI usage: cargo run -- input.txt
fn main() {
    let filename = env::args().nth(1).expect("missing input file");

    let lines: Vec<String> = io::BufReader::new(File::open(filename).expect("file not found"))
        .lines()
        .map(|l| l.expect("line not UTF-8"))
        .filter(|l| !l.is_empty())
        .collect();

    let draws = parse_draws(&lines[0]);

    let bingo_boards = {
        let ref_lines: Vec<&str> = lines[1..].iter().map(AsRef::as_ref).collect();

        parse_bingo_boards(&ref_lines[..])
    };

    let (fst_bingo, lst_bingo) = draw_first_and_last_bingo(draws, bingo_boards);

    if let Some((n, bb)) = fst_bingo {
        println!("first bingo score: {}", (n as u32) * bb.numbers_sum());
    }

    if let Some((n, bb)) = lst_bingo {
        println!("last bingo score:  {}", (n as u32) * bb.numbers_sum());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_bingo() {
        let bb = parse_bingo_board(
            &vec![
                "29 58 10 50 19",
                "47  4 51 22 69",
                "66  5 83 82 25",
                "71 23 64 93 14",
                "80 46 76 65 33",
            ][..],
        );
        assert!(!bb.has_bingo());
    }

    #[test]
    fn bingo_by_horizontal_line() {
        let mut bb = parse_bingo_board(
            &vec![
                "29 58 10 50 19",
                "47  4 51 22 69",
                "66  5 83 82 25",
                "71 23 64 93 14",
                "80 46 76 65 33",
            ][..],
        );
        for draw in [93, 14, 71, 23, 64] {
            bb.mark_draw(draw);
        }
        assert!(bb.has_bingo());
    }

    #[test]
    fn bingo_by_vertical_line() {
        let mut bb = parse_bingo_board(
            &vec![
                "29 58 10 50 19",
                "47  4 51 22 69",
                "66  5 83 82 25",
                "71 23 64 93 14",
                "80 46 76 65 33",
            ][..],
        );
        for draw in [82, 93, 50, 22, 65] {
            bb.mark_draw(draw);
        }
        assert!(bb.has_bingo());
    }
}
