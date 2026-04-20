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

/// Recursive byte-slice glob match used internally by [`glob_match`].
pub fn glob_match_bytes(pat: &[u8], txt: &[u8]) -> bool {
    match (pat.first(), txt.first()) {
        (None, None) => true,
        (None, Some(_)) => false,
        (Some(b'*'), _) => {
            glob_match_bytes(&pat[1..], txt)
                || (!txt.is_empty() && glob_match_bytes(pat, &txt[1..]))
        }
        (Some(b'?'), Some(_)) => glob_match_bytes(&pat[1..], &txt[1..]),
        (Some(p), Some(t)) if p == t => glob_match_bytes(&pat[1..], &txt[1..]),
        _ => false,
    }
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
}
