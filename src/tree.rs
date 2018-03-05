use physics::AABox;
use std::cmp::Ordering;

#[derive(Debug, PartialEq)]
pub enum Content {
    Internal(usize, usize),
    Leaf(usize),
}

#[derive(Debug)]
pub struct Node {
    #[cfg(test)]
    cut: (usize, f64),
    pub content: Content,
    pub bounds: AABox,
}

#[derive(Debug)]
pub struct Tree(pub Vec<Node>);

impl Tree {
    pub fn new(input: Vec<[f64; 2]>) -> Tree {
        let mut tree = Tree(Vec::new());
        if !input.is_empty() {
            tree.build(
                &mut input.iter().cloned().enumerate().collect::<Vec<_>>(),
                0,
            );
        }
        tree
    }

    pub fn new_<T>(input: &Vec<([f64; 2], T)>) -> Tree {
        let mut tree = Tree(Vec::new());
        if !input.is_empty() {
            tree.build(
                &mut input
                    .iter()
                    .map(|&(p, _)| p)
                    .enumerate()
                    .collect::<Vec<_>>(),
                0,
            );
        }
        tree
    }

    fn build(
        &mut self,
        points: &mut [(usize, [f64; 2])],
        depth: usize,
    ) -> usize {
        let indent = || {
            for _ in 0..depth * 2 {
                eprint!(" ");
            }
        };
        indent();
        eprintln!("build(points={:?})", points);
        if points.len() == 1 {
            indent();
            eprintln!("Inserting leaf");
            let p = points[0].1;
            self.0.push(Node {
                #[cfg(test)]
                cut: (0, 0.0),
                content: Content::Leaf(points[0].0),
                bounds: AABox {
                    xmin: p[0] - 0.5,
                    xmax: p[0] + 0.5,
                    ymin: p[1] - 0.5,
                    ymax: p[1] + 0.5,
                },
            });
            return self.0.len() - 1;
        }

        // Compute bounds
        let mut bounds = AABox::empty();
        for p in points.iter() {
            bounds.add_square1(p.1);
        }
        indent();
        eprintln!(
            "Bounds: {}, {}, {}, {}",
            bounds.xmin, bounds.xmax, bounds.ymin, bounds.ymax
        );

        // Cut along the larger axis
        let axis = if bounds.ymax - bounds.ymin > bounds.xmax - bounds.xmin {
            indent();
            eprintln!("Cut along Y");
            1
        } else {
            indent();
            eprintln!("Cut along X");
            0
        };

        // Sort point along that axis
        points.sort_by(|a, b| {
            if a.1 == b.1 {
                Ordering::Equal
            } else if a.1 < b.1 {
                Ordering::Less
            } else {
                Ordering::Greater
            }
        });

        // Find median
        let mut median = points.len() / 2;
        indent();
        eprintln!("median index={}, value={}", median, points[median].1[axis]);
        while median + 1 < points.len()
            && points[median].1[axis] + 0.5 > points[median + 1].1[axis]
        {
            median += 1;
        }
        if median + 1 == points.len() {
            median = points.len() / 2;
            while median - 1 > 0
                && points[median].1[axis] - 0.5 < points[median - 1].1[axis]
            {
                median -= 1;
            }
        }
        indent();
        eprintln!(
            "adjusted median to index={}, value={}",
            median, points[median].1[axis]
        );
        assert!(median > 0);
        assert!(median < points.len());

        // Insert node
        let idx = self.0.len();
        self.0.push(Node {
            #[cfg(test)]
            cut: (axis, points[median].1[axis]),
            content: Content::Internal(0, 0),
            bounds: bounds,
        });
        let left = self.build(&mut points[..median], depth + 1);
        let right = self.build(&mut points[median..], depth + 1);
        self.0[idx].content = Content::Internal(left, right);
        idx
    }
}

#[cfg(test)]
mod tests {
    use super::{Content, Node, Tree};

    fn print_tree_(nodes: &[Node], idx: usize, level: usize) {
        let node = &nodes[idx];
        let indent = || {
            for _ in 0..level * 2 {
                eprint!(" ");
            }
        };
        indent();
        eprintln!("{}", idx);
        indent();
        eprintln!(
            "Bounds: {} .. {} ; {} .. {}",
            node.bounds.xmin,
            node.bounds.xmax,
            node.bounds.ymin,
            node.bounds.ymax
        );
        match node.content {
            Content::Internal(left, right) => {
                let (axis, value) = node.cut;
                indent();
                eprintln!(
                    " Cut: {} {}",
                    if axis == 0 { "X" } else { "Y" },
                    value
                );
                indent();
                eprintln!(" Left:");
                print_tree_(nodes, left, level + 1);
                indent();
                eprintln!(" Right:");
                print_tree_(nodes, right, level + 1);
            }
            Content::Leaf(d) => {
                indent();
                eprintln!("Leaf: {}", d);
            }
        }
    }

    fn print_tree(tree: &Tree) {
        eprintln!();
        if tree.0.is_empty() {
            eprintln!("empty tree");
        } else {
            print_tree_(&tree.0, 0, 0);
        }
    }

    #[test]
    fn test_empty() {
        let tree = Tree::new(vec![]);
        print_tree(&tree);
        assert!(tree.0.is_empty());
    }

    #[test]
    fn test_build() {
        let tree = Tree::new(vec![
            [0.5, 0.5],
            [99.5, 19.5],
            [12.31, 8.05],
            [41.3, 2.0],
            [39.4, 18.9],
            [89.6, 11.2],
            [77.7, 6.0],
            [82.7, 8.0],
        ]);
        print_tree(&tree);
        assert_eq!(tree.0.len(), 15);
        assert_eq!(tree.0[1].content, Content::Internal(2, 5));
        assert_eq!(tree.0[2].content, Content::Internal(3, 4));
        assert_eq!(tree.0[3].content, Content::Leaf(0));
        assert_eq!(tree.0[4].content, Content::Leaf(2));
        assert_eq!(tree.0[5].content, Content::Internal(6, 7));
        assert_eq!(tree.0[6].content, Content::Leaf(4));
        assert_eq!(tree.0[7].content, Content::Leaf(3));
        assert_eq!(tree.0[8].content, Content::Internal(9, 12));
        assert_eq!(tree.0[9].content, Content::Internal(10, 11));
        assert_eq!(tree.0[10].content, Content::Leaf(6));
        assert_eq!(tree.0[11].content, Content::Leaf(7));
        assert_eq!(tree.0[12].content, Content::Internal(13, 14));
        assert_eq!(tree.0[13].content, Content::Leaf(5));
        assert_eq!(tree.0[14].content, Content::Leaf(1));
    }
}
