use std::env;
use std::fs;
use std::io::Write;
use std::process::Command;

use anyhow::{Context, Result, bail};
use tempfile::NamedTempFile;

const NEW_TEMPLATE: &str = "\n\n\
    # Enter the todo title on the first non-comment line.\n\
    # Subsequent lines (after a blank line) become the body.\n\
    # Lines starting with '#' are ignored. An empty title aborts.\n";

const COMMENT_TEMPLATE: &str = "\n\n\
    # Enter your comment.\n\
    # Lines starting with '#' are ignored. An empty comment aborts.\n";

pub struct Composed {
    pub title: String,
    pub body: String,
}

pub fn compose_new() -> Result<Composed> {
    let raw = open_editor(NEW_TEMPLATE)?;
    Ok(parse_title_body(&raw))
}

pub fn compose_comment() -> Result<String> {
    let raw = open_editor(COMMENT_TEMPLATE)?;
    Ok(strip_comments(&raw))
}

fn open_editor(template: &str) -> Result<String> {
    let editor = pick_editor();
    // NamedTempFile uses O_EXCL with a randomised filename, so a pre-seeded
    // symlink at the target path causes creation to fail rather than letting
    // us truncate or read whatever the symlink points at. The file is
    // unlinked when `tmp` drops, even on the error paths below.
    let mut tmp = NamedTempFile::with_prefix_in("git-todo-", env::temp_dir())
        .context("creating editor temp file")?;
    tmp.write_all(template.as_bytes())
        .with_context(|| format!("writing {}", tmp.path().display()))?;

    // Exec the editor directly — no shell, no word splitting. `$EDITOR` is
    // the whole program name. Multi-word values like "code -w" won't work;
    // users who want flags should point $EDITOR at a wrapper script.
    let status = Command::new(&editor)
        .arg(tmp.path())
        .status()
        .with_context(|| format!("launching editor `{editor}`"))?;
    if !status.success() {
        bail!("editor exited with status {status}");
    }

    fs::read_to_string(tmp.path())
        .with_context(|| format!("reading {}", tmp.path().display()))
}

fn pick_editor() -> String {
    for var in ["GIT_EDITOR", "VISUAL", "EDITOR"] {
        if let Ok(v) = env::var(var) {
            if !v.is_empty() {
                return v;
            }
        }
    }
    "vi".to_string()
}

fn parse_title_body(raw: &str) -> Composed {
    let mut lines = raw
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .peekable();

    let mut title = String::new();
    for line in lines.by_ref() {
        if line.trim().is_empty() {
            if title.is_empty() {
                continue;
            } else {
                break;
            }
        }
        title = line.trim().to_string();
        break;
    }

    let mut body = String::new();
    let mut seen_content = false;
    for line in lines {
        if !seen_content && line.trim().is_empty() {
            continue;
        }
        seen_content = true;
        body.push_str(line);
        body.push('\n');
    }
    let body = body.trim_end().to_string();

    Composed { title, body }
}

fn strip_comments(raw: &str) -> String {
    let mut out = String::new();
    for line in raw.lines().filter(|l| !l.trim_start().starts_with('#')) {
        out.push_str(line);
        out.push('\n');
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_title_body_strips_comments_and_splits() {
        let raw = "# help\nBuy milk\n\nTwo litres\nAt the corner shop\n# trailing\n";
        let c = parse_title_body(raw);
        assert_eq!(c.title, "Buy milk");
        assert_eq!(c.body, "Two litres\nAt the corner shop");
    }

    #[test]
    fn parse_title_body_empty() {
        let c = parse_title_body("# only comments\n\n");
        assert!(c.title.is_empty());
        assert!(c.body.is_empty());
    }

    #[test]
    fn parse_title_body_title_only() {
        let c = parse_title_body("Solo\n# nope\n");
        assert_eq!(c.title, "Solo");
        assert!(c.body.is_empty());
    }

    #[test]
    fn strip_comments_keeps_body_only() {
        let raw = "# help\nLine one\nLine two\n# tail\n";
        assert_eq!(strip_comments(raw), "Line one\nLine two");
    }
}
