//! `OperatorOwnedMatcher` — the gitignore-style subset of rein D-153 addendum r2.
//!
//! Syntax: a literal path; a trailing `/` meaning the directory and everything under it;
//! `*` matching within one segment; `**` (a whole segment) matching zero or more segments.
//! No `!`, no character classes, no `?`, no leading `/`.
//!
//! Every pattern is anchored at the Rein home (patterns are "paths relative to the Rein
//! home", rein D-153 r2). As in gitignore, a pattern that matches a directory also covers
//! everything beneath it; a trailing-`/` pattern matches only directories at its own depth.
//! Pure function, no I/O.

/// One parsed pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Pattern {
    segments: Vec<String>,
    dir_only: bool,
}

/// Matches release paths against `layout.operator_owned`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OperatorOwnedMatcher {
    patterns: Vec<Pattern>,
}

impl OperatorOwnedMatcher {
    /// Parse the manifest's `layout.operator_owned` list. Fails on any syntax outside the subset.
    pub fn new<S: AsRef<str>>(patterns: &[S]) -> Result<Self, String> {
        let patterns = patterns
            .iter()
            .map(|p| parse(p.as_ref()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { patterns })
    }

    /// Is `path` (a file, or a directory when `is_dir`) operator-owned?
    pub fn matches(&self, path: &str, is_dir: bool) -> bool {
        let segs: Vec<&str> = path.split('/').collect();
        self.patterns
            .iter()
            .any(|p| pattern_matches(p, &segs, is_dir))
    }

    /// Is `path` itself operator-owned, or does it contain operator-owned entries?
    /// Used by the layout writer to decide whether a directory may be replaced wholesale.
    pub fn may_contain(&self, dir: &str) -> bool {
        let segs: Vec<&str> = dir.split('/').collect();
        self.patterns
            .iter()
            .any(|p| pattern_matches(p, &segs, true) || prefix_could_match(&p.segments, &segs))
    }
}

/// Bounds on one pattern (defence in depth; the matcher is polynomial regardless).
const MAX_PATTERN_LEN: usize = 256;
const MAX_STARS: usize = 8;

fn parse(raw: &str) -> Result<Pattern, String> {
    let bad = |why: &str| Err(format!("operator_owned pattern '{raw}': {why}"));
    if raw.is_empty() {
        return bad("empty");
    }
    if raw.starts_with('/') {
        return bad("leading '/' is not allowed");
    }
    if raw.len() > MAX_PATTERN_LEN {
        return bad(&format!("longer than {MAX_PATTERN_LEN} bytes"));
    }
    if raw.matches('*').count() > MAX_STARS {
        return bad(&format!("more than {MAX_STARS} '*' characters"));
    }
    if let Some(c) = raw
        .chars()
        .find(|c| matches!(c, '!' | '[' | ']' | '?' | '\\' | '\0'))
    {
        return bad(&format!("character {c:?} is not in the supported subset"));
    }
    let (body, dir_only) = match raw.strip_suffix('/') {
        Some(b) => (b, true),
        None => (raw, false),
    };
    let mut segments = Vec::new();
    for seg in body.split('/') {
        if seg.is_empty() || seg == "." || seg == ".." {
            return bad("empty, '.' or '..' segment");
        }
        segments.push(seg.to_owned());
    }
    Ok(Pattern { segments, dir_only })
}

/// Does the pattern match `segs` itself or one of its ancestor directories?
fn pattern_matches(p: &Pattern, segs: &[&str], is_dir: bool) -> bool {
    // Ancestors (proper prefixes) are always directories.
    for len in 1..segs.len() {
        if match_segments(&p.segments, &segs[..len]) {
            return true;
        }
    }
    (is_dir || !p.dir_only) && match_segments(&p.segments, segs)
}

/// Full match of pattern segments against path segments, with `**` spanning segments.
fn match_segments(pat: &[String], path: &[&str]) -> bool {
    wildcard_match(pat, path, |p| p == "**", |p, s| match_segment(p, s))
}

/// Iterative wildcard match with single-point backtracking: O(len(pat) × len(s)) worst
/// case, never exponential (security review E-3 P1-1). `is_star` marks the token that
/// matches any run of items; `eq` compares one pattern token with one item.
fn wildcard_match<P, S>(
    pat: &[P],
    s: &[S],
    is_star: impl Fn(&P) -> bool,
    eq: impl Fn(&P, &S) -> bool,
) -> bool {
    let (mut pi, mut si) = (0, 0);
    let mut resume: Option<(usize, usize)> = None; // (star index, item index it covers up to)
    while si < s.len() {
        if pi < pat.len() && is_star(&pat[pi]) {
            resume = Some((pi, si));
            pi += 1;
        } else if pi < pat.len() && eq(&pat[pi], &s[si]) {
            pi += 1;
            si += 1;
        } else if let Some((star, covered)) = resume {
            pi = star + 1;
            si = covered + 1;
            resume = Some((star, covered + 1));
        } else {
            return false;
        }
    }
    pat[pi..].iter().all(is_star)
}

/// Could some descendant of `dir` match `pat`? (`dir` is a proper prefix of a possible match.)
fn prefix_could_match(pat: &[String], dir: &[&str]) -> bool {
    match (pat.split_first(), dir.split_first()) {
        (_, None) => !pat.is_empty(),
        (None, Some(_)) => false,
        (Some((first, _)), Some(_)) if first == "**" => true,
        (Some((first, rest)), Some((seg, tail))) => {
            match_segment(first, seg) && prefix_could_match(rest, tail)
        }
    }
}

/// Glob match of one segment; `*` matches any run of characters (never `/`).
fn match_segment(pat: &str, seg: &str) -> bool {
    if !pat.contains('*') {
        return pat == seg;
    }
    let p: Vec<char> = pat.chars().collect();
    let s: Vec<char> = seg.chars().collect();
    wildcard_match(&p, &s, |c| *c == '*', |a, b| a == b)
}

#[cfg(test)]
mod tests {
    use super::OperatorOwnedMatcher;

    /// The schema-1 list from rein D-153 addendum r2 (the gate override file is named by
    /// rein; `gate-override.json` stands in for it here).
    fn schema1() -> OperatorOwnedMatcher {
        OperatorOwnedMatcher::new(&[
            "accounts.json",
            "gate-override.json",
            "secrets/",
            "run/",
            "memory/",
            "incidents.ndjson",
            "telemetry.sqlite",
            "INSIGHTS.md",
            "memory-palace.md",
            "config/rein-workspace.txt",
            "config/install-stamp.json",
            "mcp/*/node_modules/",
            "mcp/*/package-lock.json",
        ])
        .unwrap()
    }

    #[test]
    fn trailing_slash_matches_directories_only_at_its_own_depth() {
        let m = OperatorOwnedMatcher::new(&["secrets/"]).unwrap();
        assert!(m.matches("secrets", true));
        assert!(!m.matches("secrets", false));
        assert!(m.matches("secrets/f", false));
    }

    #[test]
    fn double_star_spans_zero_or_more_segments() {
        let m = OperatorOwnedMatcher::new(&["a/**/z.txt"]).unwrap();
        for p in ["a/z.txt", "a/b/z.txt", "a/b/c/z.txt"] {
            assert!(m.matches(p, false), "{p}");
        }
        assert!(!m.matches("b/z.txt", false));
    }

    #[test]
    fn may_contain_flags_directories_holding_operator_files() {
        let m = schema1();
        assert!(m.may_contain("mcp"));
        assert!(m.may_contain("config"));
        assert!(!m.may_contain("bin"));
        assert!(!m.may_contain("hooks"));
    }

    #[test]
    fn pathological_patterns_finish_quickly() {
        // E-3 security review P1-1: these were exponential under recursive backtracking.
        let seg = "a".repeat(60);
        let m = OperatorOwnedMatcher::new(&["*a*a*a*a*a*a*ab"]).unwrap();
        assert!(!m.matches(&seg, false));
        // Below the parser's cap, straight into the matcher: 12 `**` over 40 segments.
        let deep = vec!["a"; 40];
        let mut pat: Vec<String> = vec!["**".to_owned(); 12];
        pat.push("z".to_owned());
        assert!(!super::match_segments(&pat, &deep));
        assert!(OperatorOwnedMatcher::new(&["*a*a*a*a*a*a*a*a*a"]).is_err());
        assert!(OperatorOwnedMatcher::new(&["a".repeat(257)]).is_err());
    }

    #[test]
    fn rejects_syntax_outside_the_subset() {
        for p in ["", "/abs", "!neg", "a/[ab]", "a?", "a//b", "../x", "a\\b"] {
            assert!(OperatorOwnedMatcher::new(&[p]).is_err(), "{p:?}");
        }
    }
}
