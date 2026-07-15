//! Finds the diff of two `str`s using the Myer's diff algorithm with linear space.
//! Based on https://blog.jcoglan.com/2017/02/17/the-myers-diff-algorithm-part-3/

use std::{
    fmt::{Debug, Display},
    ops::{Index, IndexMut},
};

use anyhow::Result;

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Diff<'a> {
    Delete(&'a str),
    Insert(&'a str),
    Equal(&'a str),
}

impl Debug for Diff<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Delete(line) => f.debug_tuple("Delete").field(line).finish(),
            Self::Insert(line) => f.debug_tuple("Insert").field(line).finish(),
            Self::Equal(line) => f.debug_tuple("Equal").field(line).finish(),
        }
    }
}

#[derive(Debug)]
struct MyersBox {
    left: usize,
    top: usize,
    right: usize,
    bottom: usize,
}

impl MyersBox {
    fn width(&self) -> isize {
        self.right as isize - self.left as isize
    }

    fn height(&self) -> isize {
        self.bottom as isize - self.top as isize
    }

    fn size(&self) -> isize {
        self.width() + self.height()
    }

    fn delta(&self) -> isize {
        self.width() - self.height()
    }
}

#[derive(Debug)]
struct Snake {
    start: (isize, isize),
    end: (isize, isize),
}

/// Handles negative index values as per Myers's algorithm.
/// For example, when the max depth is 3, k values range from -d (-3) to d (3).
/// Recall that k = x - y, so when k = d, it is all the way to the right on the diff
/// graph (high x value). And when k = -d, it is all the way to the left (high y value).
///
///   k
///    -3 -2 -1  0  1  2  3
/// d 0
///   1
///   2
///   3
///
/// In a MyersVector.v Vec, the k values index into:
/// idx:   0   1   2  3  4  5  6
///   k: [-3, -2, -1, 0, 1, 2, 3]
///
/// To convert k to the correct index, add the maximum depth value.
///
/// Inspired by Armin Ronacher's implementation as part of Similar:
/// https://github.com/mitsuhiko/similar/blob/main/src/algorithms/myers.rs#L150
#[derive(Debug)]
struct MyersVector {
    v: Vec<isize>,
    max_d: isize,
}

impl MyersVector {
    fn new(max_d: usize) -> Self {
        Self {
            v: vec![-1; 2 * max_d + 1],
            max_d: max_d as isize,
        }
    }
}

impl IndexMut<isize> for MyersVector {
    fn index_mut(&mut self, index: isize) -> &mut Self::Output {
        &mut self.v[(index + self.max_d) as usize]
    }
}

impl Index<isize> for MyersVector {
    type Output = isize;

    fn index(&self, index: isize) -> &Self::Output {
        &self.v[(index + self.max_d) as usize]
    }
}

impl Display for MyersVector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let v = self.v.clone();
        let Some((last, elements)) = v.split_last() else {
            return write!(f, "[]");
        };

        let mut buf = String::new();
        buf.push_str("[");
        for k in elements {
            buf.push_str(&k.to_string());
            buf.push_str(", ");
        }
        buf.push_str(&last.to_string());
        buf.push_str("]");
        write!(f, "{buf}")
    }
}

/// Entry point for creating the diff.
pub fn diff<'a>(current: &'a Vec<&str>, target: &'a Vec<&str>) -> Result<Vec<Diff<'a>>> {
    let mut diff = vec![];

    // TODO: handle equal leading and ending lines.

    let Some(steps) = walk_snakes(current, target) else {
        // Failed to compute the diff, so fall back on original Gitlet approach: current file
        // contents followed by the target's contents. I'm treating the current as deleted and the
        // target as inserted.
        for line in current.iter() {
            diff.push(Diff::Delete(&line));
        }

        for line in target.iter() {
            diff.push(Diff::Insert(&line));
        }

        return Ok(diff);
    };

    for ((x1, y1), (x2, y2)) in steps.iter() {
        if x1 == x2 {
            diff.push(Diff::Insert(&target[*y1]));
        } else if y1 == y2 {
            diff.push(Diff::Delete(&current[*x1]));
        } else {
            diff.push(Diff::Equal(&current[*x1]));
        }
    }

    Ok(diff)
}

/// Returns the complete path of the shortest edit sequence.
fn walk_snakes<'a>(
    current: &'a Vec<&str>,
    target: &'a Vec<&str>,
) -> Option<Vec<((usize, usize), (usize, usize))>> {
    let Some(steps) = find_path(current, target, 0, 0, current.len(), target.len()) else {
        return None;
    };

    let mut sequence = vec![];

    for ((x1, y1), (x2, y2)) in steps.iter().zip(steps.iter().skip(1)) {
        let (mut x1, mut y1, x2, y2) = (*x1, *y1, *x2, *y2);

        // Walk diagonals.
        while x1 < x2 && y1 < y2 && current[x1] == target[y1] {
            sequence.push(((x1, y1), (x2, y2)));
            x1 += 1;
            y1 += 1;
        }

        if y2 - y1 > x2 - x1 {
            // Downward step.
            sequence.push(((x1, y1), (x1, y1 + 1)));
            y1 += 1;
        } else if x2 - x1 > y2 - y1 {
            // Rightward step.
            sequence.push(((x1, y1), (x1 + 1, y1)));
            x1 += 1;
        }
        // Does nothing if the snake is all diagonals.

        // Check for diagonals after any down or right moves.
        while x1 < x2 && y1 < y2 && current[x1] == target[y1] {
            sequence.push(((x1, y1), (x2, y2)));
            x1 += 1;
            y1 += 1;
        }
    }

    Some(sequence)
}

fn find_path<'a>(
    current: &'a Vec<&str>,
    target: &'a Vec<&str>,
    left: usize,
    top: usize,
    right: usize,
    bottom: usize,
) -> Option<Vec<(usize, usize)>> {
    let m_box = MyersBox {
        left,
        top,
        right,
        bottom,
    };
    let Some((start, end)) = midpoint(current, target, &m_box) else {
        return None;
    };

    let mut head = if let Some(head) = find_path(current, target, left, top, start.0, start.1) {
        head
    } else {
        vec![(start.0, start.1)]
    };

    let tail = if let Some(tail) = find_path(current, target, end.0, end.1, right, bottom) {
        tail
    } else {
        vec![(end.0, end.1)]
    };

    head.extend(tail);
    Some(head)
}

fn midpoint<'a>(
    current: &'a Vec<&str>,
    target: &'a Vec<&str>,
    m_box: &MyersBox,
) -> Option<((usize, usize), (usize, usize))> {
    if m_box.size() <= 0 {
        return None;
    }

    let max_d = (m_box.size() as usize).div_ceil(2);

    let mut vf = MyersVector::new(max_d);
    vf[1] = m_box.left as isize;
    let mut vb = MyersVector::new(max_d);
    vb[1] = m_box.bottom as isize;

    for d in 0..max_d + 1 {
        if let Some(snake) = forwards(current, target, m_box, &mut vf, &mut vb, d) {
            return Some((
                (snake.start.0 as usize, snake.start.1 as usize),
                (snake.end.0 as usize, snake.end.1 as usize),
            ));
        }

        if let Some(snake) = backwards(current, target, m_box, &mut vf, &mut vb, d) {
            return Some((
                (snake.start.0 as usize, snake.start.1 as usize),
                (snake.end.0 as usize, snake.end.1 as usize),
            ));
        }
    }

    None
}

fn forwards<'a>(
    current: &'a Vec<&str>,
    target: &'a Vec<&str>,
    m_box: &MyersBox,
    vf: &mut MyersVector,
    vb: &mut MyersVector,
    d: usize,
) -> Option<Snake> {
    let odd_ses_len = m_box.delta() & 1 == 1;

    let d = d as isize;
    for k in (-d..=d).step_by(2) {
        let c = k - m_box.delta();

        let prev_x;
        let mut x = if k == -d || (k != d && vf[k - 1] < vf[k + 1]) {
            prev_x = vf[k + 1];
            prev_x
        } else {
            prev_x = vf[k - 1];
            prev_x + 1
        };

        let mut y = m_box.top as isize + (x as isize - m_box.left as isize) - k;
        let prev_y = if d == 0 || x != prev_x { y } else { y - 1 };

        // Take diagonals in the diff graph.
        while x < m_box.right as isize
            && y < m_box.bottom as isize
            && current[x as usize] == target[y as usize]
        {
            x += 1;
            y += 1;
        }

        vf[k] = x;

        if odd_ses_len && (c >= (-(d - 1)) && c <= (d - 1)) && y >= vb[c] {
            return Some(Snake {
                start: (prev_x, prev_y),
                end: (x, y),
            });
        }
    }

    None
}

fn backwards<'a>(
    current: &'a Vec<&str>,
    target: &'a Vec<&str>,
    m_box: &MyersBox,
    vf: &mut MyersVector,
    vb: &mut MyersVector,
    d: usize,
) -> Option<Snake> {
    let even_ses_len = m_box.delta() & 1 != 1;

    let d = d as isize;
    for c in (-d..=d).rev().step_by(2) {
        let k = c + m_box.delta();

        let prev_y;
        let mut y = if c == -d || (c != d && vb[c - 1] > vb[c + 1]) {
            prev_y = vb[c + 1];
            prev_y
        } else {
            prev_y = vb[c - 1];
            prev_y - 1
        };

        let mut x = m_box.left as isize + (y - m_box.top as isize) + k;
        let prev_x = if d == 0 || y != prev_y { x } else { x + 1 };

        while x > m_box.left as isize
            && y > m_box.top as isize
            && current[x as usize - 1] == target[y as usize - 1]
        {
            x -= 1;
            y -= 1;
        }

        vb[c] = y;

        if even_ses_len && k >= (-(d - 1)) && k <= (d - 1) && x <= vf[k] {
            return Some(Snake {
                start: (x, y),
                end: (prev_x, prev_y),
            });
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_myer_example() -> Result<()> {
        let a = vec!["A", "B", "C", "A", "B", "B", "A"];
        let b = vec!["C", "B", "A", "B", "A", "C"];
        let actual_ses = diff(&a, &b)?;
        let expected_ses = vec![
            Diff::Delete("A"),
            Diff::Delete("B"),
            Diff::Equal("C"),
            Diff::Insert("B"),
            Diff::Equal("A"),
            Diff::Equal("B"),
            Diff::Delete("B"),
            Diff::Equal("A"),
            Diff::Insert("C"),
        ];

        assert_eq!(actual_ses, expected_ses);
        Ok(())
    }

    #[test]
    fn test_one_line_change() -> Result<()> {
        let a = vec!["Main text"];
        let b = vec!["Dev text"];
        let actual_ses = diff(&a, &b)?;
        let expected_ses = vec![Diff::Delete("Main text"), Diff::Insert("Dev text")];

        // FIX: Note that this test only passes thanks to the fallback in diff().
        assert_eq!(actual_ses, expected_ses);
        Ok(())
    }
}
