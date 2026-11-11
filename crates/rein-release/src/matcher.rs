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
//!
//! Matching is case-sensitive, as rein's case table states (D-025). The folded lookup
//! `owned_pattern(_, true)` exists only for the parse refusal of a shipped path that is an
//! owned path in another case (D-023 r1).

/// One parsed pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Pattern {
    raw: String,
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
        self.first_match(path, is_dir, false).is_some()
    }

    /// The pattern owning `path`, `path` tested as a file and as a directory, or any
    /// directory above it. `fold`: compare ASCII case-insensitively (the D-023 r1 parse
    /// refusal only); otherwise exactly as written (rein D-155 r2).
    pub fn owned_pattern(&self, path: &str, fold: bool) -> Option<&str> {
        self.first_match(path, false, fold)
            .or_else(|| self.first_match(path, true, fold))
    }

    /// The first pattern that owns `path` (as a file or a directory) or one of its ancestor
    /// directories ONLY after ASCII-folding, compared per pattern and per level (D-023 r1).
    /// A path some other pattern owns exactly is still reported: on a case-insensitive home
    /// it would overwrite the file this pattern protects (E-29 security review P2-2).
    pub fn folded_only_pattern(&self, path: &str) -> Option<&str> {
        let segs: Vec<&str> = path.split('/').collect();
        self.patterns
            .iter()
            .find(|p| {
                // A level is `segs[..n]` (a directory for n < len, the path itself at len),
                // tested as a directory, so `dir_only` never excludes it.
                let exact = full_match_levels(&p.segments, &segs, false);
                let folded = full_match_levels(&p.segments, &segs, true);
                (1..=segs.len()).any(|n| folded[n] && !exact[n])
            })
            .map(|p| p.raw.as_str())
    }

    /// The first pattern that directory `dir`, or a directory above it, lines up with (as a
    /// leading part of the pattern, or all of it) only after ASCII-folding, in some alignment
    /// (D-026): `Shared` against `shared/toolchain.sh`, `**/shared/x`, `shared/` or `shared`.
    /// Alignments are compared one by one, so an exact match elsewhere does not excuse it.
    pub fn folded_only_prefix(&self, dir: &str) -> Option<&str> {
        self.folded_only_dir(dir).map(|(_, pat)| pat)
    }

    /// [`Self::folded_only_prefix`], also naming the shortest offending directory of `dir`.
    pub fn folded_only_dir<'d>(&self, dir: &'d str) -> Option<(&'d str, &str)> {
        let segs: Vec<&str> = dir.split('/').collect();
        let end = |n: usize| segs[..n].iter().map(|s| s.len()).sum::<usize>() + n - 1;
        self.patterns
            .iter()
            .filter_map(|p| aligns_only_folded(&p.segments, &segs).map(|n| (n, p.raw.as_str())))
            .min_by_key(|(n, _)| *n)
            .map(|(n, pat)| (&dir[..end(n)], pat))
    }

    fn first_match(&self, path: &str, is_dir: bool, fold: bool) -> Option<&str> {
        let segs: Vec<&str> = path.split('/').collect();
        self.patterns
            .iter()
            .find(|p| pattern_matches(p, &segs, is_dir, fold))
            .map(|p| p.raw.as_str())
    }

    /// Is `path` itself operator-owned, or does it contain operator-owned entries?
    /// Used by the layout writer to decide whether a directory may be replaced wholesale.
    pub fn may_contain(&self, dir: &str) -> bool {
        let segs: Vec<&str> = dir.split('/').collect();
        self.patterns.iter().any(|p| {
            pattern_matches(p, &segs, true, false) || prefix_could_match(&p.segments, &segs)
        })
    }
}

/// Bounds on one pattern (defence in depth; the matcher is polynomial regardless).
const MAX_PATTERN_LEN: usize = 256;
const MAX_STARS: usize = 8;

fn parse(raw: &str) -> Result<Pattern, String> {
    // Escaped: a pattern comes from a manifest, and its control bytes must never reach the
    // terminal (D-021 r4).
    let bad = |why: &str| {
        Err(format!(
            "operator_owned pattern '{}': {why}",
            raw.escape_debug()
        ))
    };
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
    Ok(Pattern {
        raw: raw.to_owned(),
        segments,
        dir_only,
    })
}

/// Does the pattern match `segs` itself or one of its ancestor directories?
fn pattern_matches(p: &Pattern, segs: &[&str], is_dir: bool, fold: bool) -> bool {
    // Ancestors (proper prefixes) are always directories.
    for len in 1..segs.len() {
        if match_segments(&p.segments, &segs[..len], fold) {
            return true;
        }
    }
    (is_dir || !p.dir_only) && match_segments(&p.segments, segs, fold)
}

/// Full match of pattern segments against path segments, with `**` spanning segments.
fn match_segments(pat: &[String], path: &[&str], fold: bool) -> bool {
    wildcard_match(pat, path, |p| p == "**", |p, s| match_segment(p, s, fold))
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

/// `levels[n]`: does `pat` match `segs[..n]` in full? One table, O(len(pat) × len(segs))
/// segment comparisons, never a search over alignments (E-29 security review P1-1).
fn full_match_levels(pat: &[String], segs: &[&str], fold: bool) -> Vec<bool> {
    let n = segs.len();
    // row[j] = pat[..i] matches segs[..j], for the current i.
    let mut row = vec![false; n + 1];
    row[0] = true;
    for p in pat {
        let mut next = vec![false; n + 1];
        for j in 0..=n {
            next[j] = if p == "**" {
                row[j] || (j > 0 && next[j - 1])
            } else {
                j > 0 && row[j - 1] && match_segment(p, segs[j - 1], fold)
            };
        }
        row = next;
    }
    row
}

/// Does `dir`, or a directory above it, line up with the leading segments of `pat` (`**`
/// consuming zero or more segments) in an alignment where at least one segment is equal only
/// after ASCII-folding? A reachability table over (pattern position, dir position, differs):
/// O(len(pat) × len(dir)) states, each visited once (E-29 security review P1-1).
fn aligns_only_folded(pat: &[String], dir: &[&str]) -> Option<usize> {
    let (m, n) = (pat.len(), dir.len());
    // Two columns of reach[i][d] (pat[..i] aligned with dir[..j], `d` = some segment
    // differed in case): every move goes from column j to j or j + 1, so memory is O(m)
    // (E-29 round-5 P3-1).
    let mut cur = vec![[false; 2]; m + 1];
    cur[0][0] = true;
    for j in 0..=n {
        let seg = dir.get(j).copied(); // None once the whole dir is aligned
        let mut next = vec![[false; 2]; m + 1];
        for i in 0..=m {
            for d in 0..2 {
                if !cur[i][d] || i == m {
                    continue;
                }
                let p = &pat[i];
                if p == "**" {
                    cur[i + 1][d] = true; // `**` covers nothing more
                    if seg.is_some() {
                        next[i][d] = true; // `**` covers dir[j] too
                    }
                } else if let Some(seg) = seg {
                    if match_segment(p, seg, false) {
                        next[i + 1][d] = true;
                    } else if match_segment(p, seg, true) {
                        next[i + 1][1] = true;
                    }
                }
            }
        }
        // The shortest dir prefix (j >= 1) reached with a case difference.
        if j >= 1 && cur.iter().any(|r| r[1]) {
            return Some(j);
        }
        cur = next;
    }
    None
}

/// Could some descendant of `dir` match `pat`? (`dir` is a proper prefix of a possible match.)
fn prefix_could_match(pat: &[String], dir: &[&str]) -> bool {
    match (pat.split_first(), dir.split_first()) {
        (_, None) => !pat.is_empty(),
        (None, Some(_)) => false,
        (Some((first, _)), Some(_)) if first == "**" => true,
        (Some((first, rest)), Some((seg, tail))) => {
            match_segment(first, seg, false) && prefix_could_match(rest, tail)
        }
    }
}

/// Glob match of one segment; `*` matches any run of characters (never `/`).
fn match_segment(pat: &str, seg: &str, fold: bool) -> bool {
    if !pat.contains('*') {
        return if fold {
            pat.eq_ignore_ascii_case(seg)
        } else {
            pat == seg
        };
    }
    let p: Vec<char> = pat.chars().collect();
    let s: Vec<char> = seg.chars().collect();
    let eq = |a: &char, b: &char| {
        if fold {
            a.eq_ignore_ascii_case(b)
        } else {
            a == b
        }
    };
    wildcard_match(&p, &s, |c| *c == '*', eq)
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
        assert!(!super::match_segments(&pat, &deep, true));
        assert!(OperatorOwnedMatcher::new(&["*a*a*a*a*a*a*a*a*a"]).is_err());
        assert!(OperatorOwnedMatcher::new(&["a".repeat(257)]).is_err());
    }

    #[test]
    fn matching_is_case_sensitive_and_only_the_folded_lookup_folds() {
        let m = OperatorOwnedMatcher::new(&["shared/toolchain.sh", "mcp/*/node_modules/"]).unwrap();
        assert!(!m.matches("shared/Toolchain.sh", false), "D-025");
        assert!(!m.matches("MCP/x/Node_Modules/a.js", false));
        assert_eq!(
            m.owned_pattern("MCP/x/Node_Modules/a.js", true),
            Some("mcp/*/node_modules/")
        );
        // D-026: a directory that starts an owned pattern only in another case.
        let lib =
            OperatorOwnedMatcher::new(&["shared/lib/x", "mcp/*/node_modules/", "**/c/"]).unwrap();
        assert_eq!(lib.folded_only_prefix("Shared"), Some("shared/lib/x"));
        assert_eq!(lib.folded_only_prefix("shared/Lib"), Some("shared/lib/x"));
        assert_eq!(lib.folded_only_prefix("shared/lib"), None);
        assert_eq!(
            lib.folded_only_prefix("mcp/AnyServer"),
            None,
            "a glob matches any case"
        );
        assert_eq!(lib.folded_only_prefix("MCP"), Some("mcp/*/node_modules/"));
        // E-29 round 3: `**` first, a dir-only pattern and a literal equal to the root.
        for pat in [
            "**/shared/toolchain.sh",
            "shared/",
            "shared",
            "**/**/shared/x",
        ] {
            let m = OperatorOwnedMatcher::new(&[pat]).unwrap();
            assert_eq!(m.folded_only_prefix("Shared"), Some(pat), "{pat}");
            assert_eq!(m.folded_only_prefix("shared"), None, "{pat}");
        }
        // E-29 round 4 P1-1: no search over alignments; a deep path stays fast.
        let stars = OperatorOwnedMatcher::new(&["**/**/**/**/zzz"]).unwrap();
        let deep = vec!["a"; 2000].join("/");
        let t = std::time::Instant::now();
        assert_eq!(stars.folded_only_prefix(&deep), None);
        assert_eq!(stars.folded_only_pattern(&format!("{deep}/f")), None);
        assert!(
            t.elapsed() < std::time::Duration::from_secs(1),
            "{:?}",
            t.elapsed()
        );
        let bin = OperatorOwnedMatcher::new(&["Bin"]).unwrap();
        assert_eq!(bin.folded_only_prefix("bin"), Some("Bin"));
        let lib = OperatorOwnedMatcher::new(&["shared/lib/x"]).unwrap();
        assert_eq!(
            lib.folded_only_dir("shared/Lib/deep/er"),
            Some(("shared/Lib", "shared/lib/x"))
        );
        // Per pattern and per level (E-29 security review P2-2).
        let deep = OperatorOwnedMatcher::new(&["**/cache/"]).unwrap();
        assert_eq!(
            deep.folded_only_pattern("a/Cache/b/cache/x"),
            Some("**/cache/")
        );
        assert_eq!(deep.folded_only_pattern("a/cache/b/cache/x"), None);
        let mixed = OperatorOwnedMatcher::new(&["shared/toolchain.sh", "shared/T*"]).unwrap();
        assert_eq!(
            mixed.folded_only_pattern("shared/Toolchain.sh"),
            Some("shared/toolchain.sh")
        );
        assert_eq!(
            m.owned_pattern("shared/Toolchain.sh", true),
            Some("shared/toolchain.sh")
        );
        assert_eq!(m.owned_pattern("shared/Toolchain.sh", false), None);
        assert_eq!(
            m.owned_pattern("mcp/x/node_modules/a.js", false),
            Some("mcp/*/node_modules/")
        );
        assert!(!m.matches("shared/toolchain.shx", false));
    }

    #[test]
    fn rejects_syntax_outside_the_subset() {
        for p in ["", "/abs", "!neg", "a/[ab]", "a?", "a//b", "../x", "a\\b"] {
            assert!(OperatorOwnedMatcher::new(&[p]).is_err(), "{p:?}");
        }
        // D-021 r4: a control byte from a manifest never reaches the terminal raw.
        let err = OperatorOwnedMatcher::new(&["\u{1b}[2J/../x"]).unwrap_err();
        assert!(!err.contains('\u{1b}') && err.contains("\\u{1b}"), "{err}");
    }
}
