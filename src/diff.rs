//! Finds the diff of two `str`s using the Myer's diff algorithm.

use std::fmt::Debug;

#[derive(Copy, Clone, PartialEq, Eq)]
enum Diff<'a> {
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

pub(crate) fn diff<'a>(current: &'a str, target: &'a str) -> Vec<Diff<'a>> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_myer_example() {
        let a = "A\nB\nC\nA\nB\nB\nA";
        let b = "C\nB\nA\nB\nA\nC";
        let actual_ses = diff(a, b);
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
    }
}
