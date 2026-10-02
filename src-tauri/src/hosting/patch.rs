//! Parse unified-diff text (the `patch` field of GitHub/Gitea PR files, the
//! `diff` field of GitLab MR changes) into the same hunk structure the local
//! diff viewer renders.

use crate::git::reader::{DiffHunk, DiffLine};

/// Parse `@@ -a,b +c,d @@ section` into (old_start, old_lines, new_start, new_lines).
fn parse_hunk_header(line: &str) -> Option<(u32, u32, u32, u32)> {
    let rest = line.strip_prefix("@@ ")?;
    let end = rest.find(" @@")?;
    let mut parts = rest[..end].split_whitespace();
    let old = parts.next()?.strip_prefix('-')?;
    let new = parts.next()?.strip_prefix('+')?;
    let range = |s: &str| -> Option<(u32, u32)> {
        match s.split_once(',') {
            Some((a, b)) => Some((a.parse().ok()?, b.parse().ok()?)),
            None => Some((s.parse().ok()?, 1)),
        }
    };
    let (os, ol) = range(old)?;
    let (ns, nl) = range(new)?;
    Some((os, ol, ns, nl))
}

/// Parse a unified diff body. File headers (`diff --git`, `---`, `+++`,
/// `index`, ...) before the first hunk are skipped. Line contents keep a
/// trailing `\n` like libgit2 output; "\ No newline at end of file" becomes
/// an `=` pseudo-line, which the viewer attaches to the previous line.
pub fn parse_unified_patch(patch: &str) -> Vec<DiffHunk> {
    let mut hunks: Vec<DiffHunk> = Vec::new();
    let mut old_no = 0u32;
    let mut new_no = 0u32;

    for raw in patch.split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if line.starts_with("@@ ") {
            if let Some((os, ol, ns, nl)) = parse_hunk_header(line) {
                old_no = os;
                new_no = ns;
                hunks.push(DiffHunk {
                    header: format!("{line}\n"),
                    old_start: os,
                    old_lines: ol,
                    new_start: ns,
                    new_lines: nl,
                    lines: Vec::new(),
                });
                continue;
            }
        }
        let Some(hunk) = hunks.last_mut() else {
            continue; // preamble before the first hunk
        };
        let mut chars = line.chars();
        let (origin, old, new) = match chars.next() {
            Some('+') => ("+", None, Some(new_no)),
            Some('-') => ("-", Some(old_no), None),
            Some(' ') => (" ", Some(old_no), Some(new_no)),
            Some('\\') => ("=", None, None),
            // Empty line: a trailing newline of the whole patch, or a
            // context line whose leading space was stripped by a tool.
            None => {
                let done = hunk.lines.iter().filter(|l| l.origin != "+" && l.origin != "=").count()
                    as u32
                    >= hunk.old_lines
                    && hunk.lines.iter().filter(|l| l.origin != "-" && l.origin != "=").count()
                        as u32
                        >= hunk.new_lines;
                if done {
                    continue;
                }
                (" ", Some(old_no), Some(new_no))
            }
            // A new file section in a multi-file diff.
            Some(_) => continue,
        };
        if old.is_some() {
            old_no += 1;
        }
        if new.is_some() {
            new_no += 1;
        }
        let content = if origin == "=" {
            format!("{}\n", line)
        } else {
            format!("{}\n", chars.as_str())
        };
        hunk.lines.push(DiffLine {
            origin: origin.to_string(),
            old_lineno: old,
            new_lineno: new,
            content,
        });
    }
    hunks
}

/// One file section of a multi-file `git diff` text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffSection {
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    /// `added`, `deleted`, `renamed`, `modified`.
    pub status: String,
    pub is_binary: bool,
    /// The section text from the first hunk on.
    pub body: String,
}

/// Strip the `a/` / `b/` prefix (and surrounding quotes) of a diff path.
fn diff_path(p: &str) -> Option<String> {
    let p = p.trim().trim_matches('"');
    if p == "/dev/null" {
        return None;
    }
    Some(
        p.strip_prefix("a/")
            .or_else(|| p.strip_prefix("b/"))
            .unwrap_or(p)
            .to_string(),
    )
}

/// Split full `git diff` output (e.g. a Gitea `pulls/{n}.diff`) into
/// per-file sections.
pub fn split_git_diff(text: &str) -> Vec<DiffSection> {
    let mut out: Vec<DiffSection> = Vec::new();
    let mut in_body = false;
    for line in text.split_inclusive('\n') {
        let bare = line.trim_end_matches(['\n', '\r']);
        if let Some(rest) = bare.strip_prefix("diff --git ") {
            // `a/old b/new` — best effort for unquoted paths without " b/".
            let (old, new) = match rest.find(" b/") {
                Some(i) => (diff_path(&rest[..i]), diff_path(&rest[i + 1..])),
                None => (None, None),
            };
            out.push(DiffSection {
                old_path: old,
                new_path: new,
                status: "modified".into(),
                is_binary: false,
                body: String::new(),
            });
            in_body = false;
            continue;
        }
        let Some(sec) = out.last_mut() else { continue };
        if in_body || bare.starts_with("@@ ") {
            in_body = true;
            sec.body.push_str(line);
            continue;
        }
        if bare.starts_with("new file mode") {
            sec.status = "added".into();
        } else if bare.starts_with("deleted file mode") {
            sec.status = "deleted".into();
        } else if let Some(p) = bare.strip_prefix("rename from ") {
            sec.status = "renamed".into();
            sec.old_path = Some(p.to_string());
        } else if let Some(p) = bare.strip_prefix("rename to ") {
            sec.new_path = Some(p.to_string());
        } else if bare.starts_with("Binary files ") || bare == "GIT binary patch" {
            sec.is_binary = true;
        } else if let Some(p) = bare.strip_prefix("--- ") {
            if sec.status != "renamed" {
                sec.old_path = diff_path(p);
            }
        } else if let Some(p) = bare.strip_prefix("+++ ") {
            if sec.status != "renamed" {
                sec.new_path = diff_path(p);
            }
        }
    }
    for sec in &mut out {
        if sec.status == "modified" {
            if sec.old_path.is_none() && sec.new_path.is_some() {
                sec.status = "added".into();
            } else if sec.new_path.is_none() && sec.old_path.is_some() {
                sec.status = "deleted".into();
            }
        }
    }
    out
}

/// Count added / deleted lines of parsed hunks.
pub fn count_changes(hunks: &[DiffHunk]) -> (u32, u32) {
    hunks.iter().flat_map(|h| &h.lines).fold((0, 0), |(a, d), l| match l.origin.as_str() {
        "+" => (a + 1, d),
        "-" => (a, d + 1),
        _ => (a, d),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_multi_file_diff() {
        let text = "diff --git a/src/a.rs b/src/a.rs\nindex 1..2 100644\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1 +1 @@\n-x\n+y\n\
diff --git a/new.txt b/new.txt\nnew file mode 100644\n--- /dev/null\n+++ b/new.txt\n@@ -0,0 +1 @@\n+hi\n\
diff --git a/old b/renamed\nsimilarity index 90%\nrename from old\nrename to renamed\n\
diff --git a/img.png b/img.png\ndeleted file mode 100644\nBinary files a/img.png and /dev/null differ\n";
        let secs = split_git_diff(text);
        assert_eq!(secs.len(), 4);
        assert_eq!(secs[0].new_path.as_deref(), Some("src/a.rs"));
        assert_eq!(secs[0].status, "modified");
        assert_eq!(parse_unified_patch(&secs[0].body).len(), 1);
        assert_eq!(count_changes(&parse_unified_patch(&secs[0].body)), (1, 1));
        assert_eq!(secs[1].status, "added");
        assert_eq!(secs[1].old_path, None);
        assert_eq!(secs[2].status, "renamed");
        assert_eq!(secs[2].old_path.as_deref(), Some("old"));
        assert_eq!(secs[2].new_path.as_deref(), Some("renamed"));
        assert!(secs[2].body.is_empty());
        assert_eq!(secs[3].status, "deleted");
        assert!(secs[3].is_binary);
    }

    #[test]
    fn parses_hunks_and_line_numbers() {
        let patch = "@@ -1,3 +1,4 @@ fn main()\n a\n-b\n+B\n+C\n c\n@@ -10 +11,0 @@\n-gone";
        let hunks = parse_unified_patch(patch);
        assert_eq!(hunks.len(), 2);
        let h = &hunks[0];
        assert_eq!((h.old_start, h.old_lines, h.new_start, h.new_lines), (1, 3, 1, 4));
        assert_eq!(h.header, "@@ -1,3 +1,4 @@ fn main()\n");
        let got: Vec<_> = h
            .lines
            .iter()
            .map(|l| (l.origin.as_str(), l.old_lineno, l.new_lineno, l.content.as_str()))
            .collect();
        assert_eq!(
            got,
            vec![
                (" ", Some(1), Some(1), "a\n"),
                ("-", Some(2), None, "b\n"),
                ("+", None, Some(2), "B\n"),
                ("+", None, Some(3), "C\n"),
                (" ", Some(3), Some(4), "c\n"),
            ]
        );
        let h2 = &hunks[1];
        assert_eq!((h2.old_start, h2.old_lines, h2.new_start, h2.new_lines), (10, 1, 11, 0));
        assert_eq!(h2.lines[0].old_lineno, Some(10));
    }

    #[test]
    fn handles_headers_no_eol_and_crlf() {
        let patch = "diff --git a/x b/x\nindex 1..2 100644\n--- a/x\n+++ b/x\n@@ -1 +1 @@\r\n-old\r\n\\ No newline at end of file\n+new\n";
        let hunks = parse_unified_patch(patch);
        assert_eq!(hunks.len(), 1);
        let origins: Vec<_> = hunks[0].lines.iter().map(|l| l.origin.as_str()).collect();
        assert_eq!(origins, vec!["-", "=", "+"]);
        assert_eq!(hunks[0].lines[0].content, "old\n");
    }

    #[test]
    fn empty_or_garbage() {
        assert!(parse_unified_patch("").is_empty());
        assert!(parse_unified_patch("Binary files differ").is_empty());
        assert!(parse_unified_patch("@@ broken @@\n+x").is_empty());
    }
}
