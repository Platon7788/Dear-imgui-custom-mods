//! Case-insensitive glob / wildcard matching for label and tag filters.
//!
//! Supports `*` (match any substring) and `?` (match any single character).
//! Patterns without wildcards fall back to a fast substring search.

/// Case-insensitive glob match against ASCII text.
///
/// `*` matches zero or more characters; `?` matches exactly one character.
/// Patterns without wildcards use `str::contains` (no recursion).
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let pat = pattern.to_ascii_lowercase();
    let txt = text.to_ascii_lowercase();
    if !pat.contains('*') && !pat.contains('?') {
        return txt.contains(pat.as_str());
    }
    glob_match_bytes(pat.as_bytes(), txt.as_bytes())
}

/// Iterative byte-slice glob match used internally by [`glob_match`].
///
/// Walks two cursors (one over the pattern, one over the text). When it
/// hits a `*`, it remembers the position and tentatively consumes nothing;
/// on a later mismatch it backtracks to the saved position and consumes
/// one more text byte. This is the classical iterative algorithm — O(P×T)
/// worst case, no stack growth, safe for arbitrarily long inputs.
pub fn glob_match_bytes(pat: &[u8], txt: &[u8]) -> bool {
    let (mut p, mut t) = (0usize, 0usize);
    let mut star_pat: Option<usize> = None;
    let mut star_txt: usize = 0;

    while t < txt.len() {
        if p < pat.len() && (pat[p] == b'?' || pat[p] == txt[t]) {
            p += 1;
            t += 1;
        } else if p < pat.len() && pat[p] == b'*' {
            star_pat = Some(p);
            star_txt = t;
            p += 1;
        } else if let Some(sp) = star_pat {
            // Backtrack: extend `*` by one text byte.
            p = sp + 1;
            star_txt += 1;
            t = star_txt;
        } else {
            return false;
        }
    }
    // Consume any trailing `*`s in the pattern.
    while p < pat.len() && pat[p] == b'*' {
        p += 1;
    }
    p == pat.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn star_matches_substring() {
        assert!(glob_match("func*call", "function_call"));
    }

    #[test]
    fn question_matches_one_char() {
        assert!(glob_match("node?", "node1"));
        assert!(!glob_match("node?", "node12"));
    }

    #[test]
    fn no_wildcard_falls_back_to_contains() {
        assert!(glob_match("beta", "alpha_beta_gamma"));
    }

    #[test]
    fn case_insensitive() {
        assert!(glob_match("HELLO", "say hello world"));
    }

    #[test]
    fn empty_pattern_matches_empty_text() {
        assert!(glob_match("", ""));
    }

    #[test]
    fn star_at_end_matches_anything() {
        assert!(glob_match("foo*", "foobar"));
        assert!(glob_match("foo*", "foo"));
        assert!(!glob_match("foo*", "fo"));
    }

    #[test]
    fn multiple_stars_backtrack_correctly() {
        // The iterative algorithm must backtrack over each `*` independently.
        assert!(glob_match("a*b*c", "axxxbyyyc"));
        assert!(glob_match("*foo*bar*", "xxfooyybarzz"));
        assert!(!glob_match("a*b*c", "axxxbyyy"));
    }

    #[test]
    fn long_input_no_stack_overflow() {
        // Pathological pattern that would have blown the recursive version's
        // stack — the iterative one is unconditionally safe.
        let txt = "a".repeat(10_000);
        let pat = "*".repeat(50) + "a";
        assert!(glob_match(&pat, &txt));
    }
}
