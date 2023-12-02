use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::File;
use std::io::{self, BufRead};
use std::ptr;
use std::rc::Rc;
use std::str::FromStr;

#[derive(Debug)]
enum ParseCaveGraphError {
    UnexpectedNumNodesInLink(usize),
    MissingStartNode,
    MissingEndNode,
}

#[derive(Debug)]
struct CaveLink {
    node_a: String,
    node_b: String,
}

impl CaveLink {
    fn new(node_a: String, node_b: String) -> CaveLink {
        CaveLink { node_a, node_b }
    }
}

impl FromStr for CaveLink {
    type Err = ParseCaveGraphError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let nodes: Vec<&str> = s.splitn(2, '-').collect();
        if nodes.len() == 2 {
            Ok(CaveLink::new(nodes[0].to_string(), nodes[1].to_string()))
        } else {
            Err(ParseCaveGraphError::UnexpectedNumNodesInLink(nodes.len()))
        }
    }
}

#[derive(Debug, PartialEq)]
enum NodeKind {
    StartCave,
    EndCave,
    BigCave,
    SmallCave,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct Node(String);

impl Node {
    fn kind(&self) -> NodeKind {
        if self.0 == "start" {
            NodeKind::StartCave
        } else if self.0 == "end" {
            NodeKind::EndCave
        } else if self.0.chars().next().unwrap().is_uppercase() {
            NodeKind::BigCave
        } else {
            NodeKind::SmallCave
        }
    }
}

#[derive(Debug)]
struct CaveGraph {
    start: Rc<Node>,
    end: Rc<Node>,
    graph: HashMap<Rc<Node>, HashSet<Rc<Node>>>,
}

impl CaveGraph {
    fn paths_with_small_caves_once(&self) -> HashSet<Vec<Node>> {
        self.paths(|path, n| !path.contains(&n))
    }

    fn paths_with_one_small_cave_twice(&self) -> HashSet<Vec<Node>> {
        self.paths(|path, n| {
            let mut node_occurences: HashMap<&Node, usize> = HashMap::new();
            for n in path.iter().filter(|n| n.kind() == NodeKind::SmallCave) {
                let num = node_occurences.entry(n).or_insert(0);
                *num += 1;
            }
            let n_occ = *node_occurences.get(n).unwrap_or(&0);
            if n_occ == 0 {
                return true;
            }
            n_occ == 1 && node_occurences.values().all(|&num| num < 2)
        })
    }

    fn paths<F>(&self, include_small_cave: F) -> HashSet<Vec<Node>>
    where
        F: Fn(&Vec<&Node>, &Node) -> bool,
    {
        let mut result: Vec<Vec<&Node>> = vec![];

        let mut visit_paths_next: Vec<Vec<&Node>> = vec![vec![&self.start]];

        while let Some(curr_path) = visit_paths_next.pop() {
            let curr_node = *curr_path.last().unwrap();

            if curr_node == Rc::as_ref(&self.end) {
                result.push(curr_path);
                continue;
            }

            for neighbour in self
                .graph
                .get(curr_node)
                .unwrap()
                .iter()
                .filter(|n| n.kind() != NodeKind::StartCave)
            {
                if neighbour.kind() == NodeKind::SmallCave
                    && !include_small_cave(&curr_path, Rc::as_ref(neighbour))
                {
                    continue;
                }

                let candidate_path = {
                    let mut v = curr_path.clone();
                    v.push(neighbour);
                    v
                };

                if result.iter().any(|p| {
                    p.len() == candidate_path.len()
                        // compare pointers for faster equivalence check
                        && p[0..candidate_path.len()]
                            .iter()
                            .zip(candidate_path[..].iter())
                            .all(|(&a, &b)| ptr::eq(a, b))
                }) {
                    continue;
                }

                visit_paths_next.push(candidate_path);
            }
        }

        result
            .iter()
            .map(|p| p.iter().map(|&n| n.clone()).collect())
            .collect()
    }
}

impl FromStr for CaveGraph {
    type Err = ParseCaveGraphError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let links = s
            .lines()
            .map(|l| l.parse())
            .collect::<Result<Vec<CaveLink>, ParseCaveGraphError>>()?;
        links.try_into()
    }
}

impl TryFrom<Vec<CaveLink>> for CaveGraph {
    type Error = ParseCaveGraphError;

    fn try_from(value: Vec<CaveLink>) -> Result<Self, Self::Error> {
        let mut nodes: HashMap<&str, Rc<Node>> = HashMap::new();

        for cl in &value {
            nodes
                .entry(&cl.node_a)
                .or_insert_with(|| Rc::new(Node(cl.node_a.to_string())));

            nodes
                .entry(&cl.node_b)
                .or_insert_with(|| Rc::new(Node(cl.node_b.to_string())));
        }

        let mut graph: HashMap<Rc<Node>, HashSet<Rc<Node>>> = HashMap::new();

        for cl in &value {
            let node_a = nodes.get(cl.node_a.as_str()).unwrap();
            let node_b = nodes.get(cl.node_b.as_str()).unwrap();

            let links_a = graph.entry(Rc::clone(node_a)).or_default();
            links_a.insert(node_b.clone());

            let links_b = graph.entry(Rc::clone(node_b)).or_default();
            links_b.insert(node_a.clone());
        }

        let start_node: Rc<Node> = match graph.entry(Rc::new(Node("start".to_string()))) {
            e @ Entry::Occupied { .. } => Rc::clone(e.key()),
            _ => return Err(ParseCaveGraphError::MissingStartNode),
        };

        let end_node: Rc<Node> = match graph.entry(Rc::new(Node("end".to_string()))) {
            e @ Entry::Occupied { .. } => Rc::clone(e.key()),
            _ => return Err(ParseCaveGraphError::MissingEndNode),
        };

        Ok(CaveGraph {
            start: start_node,
            end: end_node,
            graph,
        })
    }
}

/// CLI usage: cargo run --release -- input.txt
fn main() {
    let filename = env::args().nth(1).expect("Missing input file");

    let map: CaveGraph = io::BufReader::new(File::open(filename).expect("File not found"))
        .lines()
        .map(|l| {
            let line = &l.expect("Line not UTF-8");
            line.parse()
                .unwrap_or_else(|e| panic!("Invalid edge ({:?}): {}", e, line))
        })
        .collect::<Vec<CaveLink>>()
        .try_into()
        .unwrap();

    println!(
        "Number of distinct paths with small caves visited once: {}",
        map.paths_with_small_caves_once().len(),
    );

    println!(
        "  with 1 small cave visited twice: {}",
        map.paths_with_one_small_cave_twice().len()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_paths_with_small_caves_once() {
        let cg: CaveGraph = "start-A\n\
                             A-b\n\
                             A-end"
            .parse()
            .unwrap();

        assert_eq!(
            cg.paths_with_small_caves_once(),
            HashSet::from([
                vec![
                    Node("start".to_string()),
                    Node("A".to_string()),
                    Node("end".to_string())
                ],
                vec![
                    Node("start".to_string()),
                    Node("A".to_string()),
                    Node("b".to_string()),
                    Node("A".to_string()),
                    Node("end".to_string())
                ]
            ])
        );
    }

    #[test]
    fn collect_paths_with_one_small_cave_twice() {
        let cg: CaveGraph = "start-A\n\
                             A-b\n\
                             A-end"
            .parse()
            .unwrap();

        assert_eq!(
            cg.paths_with_one_small_cave_twice(),
            HashSet::from([
                vec![
                    Node("start".to_string()),
                    Node("A".to_string()),
                    Node("end".to_string())
                ],
                vec![
                    Node("start".to_string()),
                    Node("A".to_string()),
                    Node("b".to_string()),
                    Node("A".to_string()),
                    Node("end".to_string())
                ],
                vec![
                    Node("start".to_string()),
                    Node("A".to_string()),
                    Node("b".to_string()),
                    Node("A".to_string()),
                    Node("b".to_string()),
                    Node("A".to_string()),
                    Node("end".to_string())
                ]
            ])
        );
    }
}
