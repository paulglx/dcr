//! Word-level diff of two short strings, used to highlight changed tag values.
//!
//! Tokens are alternating runs of whitespace and non-whitespace, so spacing is
//! preserved verbatim in the output. Changes are computed with a plain LCS table;
//! inputs are tag values, so quadratic cost is irrelevant.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Equal,
    Delete,
    Insert,
}

/// Splits `s` into maximal runs of whitespace / non-whitespace characters.
fn tokenize(s: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut start = 0;
    let mut iter = s.char_indices().peekable();
    while let Some((i, c)) = iter.next() {
        let ws = c.is_whitespace();
        let boundary = match iter.peek() {
            Some(&(_, next)) => next.is_whitespace() != ws,
            None => true,
        };
        if boundary {
            let end = i + c.len_utf8();
            tokens.push(&s[start..end]);
            start = end;
        }
    }
    tokens
}

/// Word diff of `old` against `new`. Equal tokens are emitted in order; at each
/// divergence deletions (from `old`) come before insertions (from `new`).
pub fn word_diff<'a>(old: &'a str, new: &'a str) -> Vec<(Change, &'a str)> {
    let a = tokenize(old);
    let b = tokenize(new);
    let (n, m) = (a.len(), b.len());

    // lcs[i][j] = length of LCS of a[i..] and b[j..]
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }

    let mut out = Vec::with_capacity(n.max(m));
    let (mut i, mut j) = (0, 0);
    while i < n || j < m {
        if i < n && j < m && a[i] == b[j] {
            out.push((Change::Equal, a[i]));
            i += 1;
            j += 1;
        } else if j >= m || (i < n && lcs[i + 1][j] >= lcs[i][j + 1]) {
            out.push((Change::Delete, a[i]));
            i += 1;
        } else {
            out.push((Change::Insert, b[j]));
            j += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::Change::*;
    use super::*;

    #[test]
    fn tokenizes_words_and_whitespace_runs() {
        assert_eq!(tokenize("a  b\tc"), vec!["a", "  ", "b", "\t", "c"]);
        assert_eq!(tokenize(""), Vec::<&str>::new());
        assert_eq!(tokenize("  "), vec!["  "]);
    }

    #[test]
    fn identical_strings_are_all_equal() {
        assert_eq!(
            word_diff("CT HEAD", "CT HEAD"),
            vec![(Equal, "CT"), (Equal, " "), (Equal, "HEAD")]
        );
    }

    #[test]
    fn replaced_word_yields_delete_then_insert() {
        assert_eq!(
            word_diff("CT HEAD", "CT CHEST"),
            vec![(Equal, "CT"), (Equal, " "), (Delete, "HEAD"), (Insert, "CHEST")]
        );
    }

    #[test]
    fn inserted_and_removed_words() {
        assert_eq!(
            word_diff("a b c", "a c"),
            vec![(Equal, "a"), (Equal, " "), (Delete, "b"), (Delete, " "), (Equal, "c")]
        );
        assert_eq!(
            word_diff("", "new"),
            vec![(Insert, "new")]
        );
        assert_eq!(
            word_diff("old", ""),
            vec![(Delete, "old")]
        );
    }

    #[test]
    fn output_reconstructs_both_inputs() {
        let (old, new) = ("Smith^John  MD", "Smith^Jane MD PhD");
        let diff = word_diff(old, new);
        let rebuilt_old: String = diff
            .iter()
            .filter(|(c, _)| *c != Insert)
            .map(|(_, t)| *t)
            .collect();
        let rebuilt_new: String = diff
            .iter()
            .filter(|(c, _)| *c != Delete)
            .map(|(_, t)| *t)
            .collect();
        assert_eq!(rebuilt_old, old);
        assert_eq!(rebuilt_new, new);
    }
}
