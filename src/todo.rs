use std::collections::HashSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const ID_LEN: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Open,
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub at: DateTime<Utc>,
    pub by: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Todo {
    pub id: String,
    pub title: String,
    pub status: Status,
    pub created: DateTime<Utc>,
    pub created_by: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub done: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub done_by: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub body: String,
    // Must stay last: TOML requires array-of-tables after all scalar fields
    // of the same parent table.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<Comment>,
}

impl Todo {
    /// Construct a new todo with a random id that isn't already in `taken`.
    /// `taken` is the set of ids currently in the store; with ID_LEN=8
    /// (32 bits) the birthday bound is ~65k todos, so a random draw can
    /// collide. We re-roll until we find a free id.
    pub fn new(
        title: String,
        body: String,
        author: String,
        created: DateTime<Utc>,
        taken: &HashSet<&str>,
    ) -> Self {
        let id = loop {
            let candidate = random_id();
            if !taken.contains(candidate.as_str()) {
                break candidate;
            }
        };
        Self {
            id,
            title,
            status: Status::Open,
            created,
            created_by: author,
            done: None,
            done_by: None,
            labels: Vec::new(),
            body,
            comments: Vec::new(),
        }
    }

    pub fn mark_done(&mut self, by: String, at: DateTime<Utc>) {
        self.status = Status::Done;
        self.done = Some(at);
        self.done_by = Some(by);
    }

    pub fn is_open(&self) -> bool {
        matches!(self.status, Status::Open)
    }

    /// Apply a list of label edits (`+foo` add, `-foo` remove). Returns
    /// the (added, removed) pair actually applied (idempotent: re-adding
    /// an existing label or removing an absent one is a no-op).
    pub fn apply_label_edits(&mut self, edits: &[LabelEdit]) -> (Vec<String>, Vec<String>) {
        let mut added = Vec::new();
        let mut removed = Vec::new();
        for edit in edits {
            match edit {
                LabelEdit::Add(name) => {
                    if !self.labels.iter().any(|l| l == name) {
                        self.labels.push(name.clone());
                        added.push(name.clone());
                    }
                }
                LabelEdit::Remove(name) => {
                    let before = self.labels.len();
                    self.labels.retain(|l| l != name);
                    if self.labels.len() != before {
                        removed.push(name.clone());
                    }
                }
            }
        }
        self.labels.sort();
        (added, removed)
    }

    pub fn add_comment(&mut self, by: String, body: String, at: DateTime<Utc>) {
        self.comments.push(Comment { at, by, body });
    }

    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }

    pub fn from_toml(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LabelEdit {
    Add(String),
    Remove(String),
}

impl LabelEdit {
    /// Parse a single token like `+foo` or `-bar`. Bare `foo` is treated as add.
    pub fn parse(token: &str) -> Result<Self, String> {
        if token.is_empty() {
            return Err("empty label".to_string());
        }
        let (kind, rest) = match token.as_bytes()[0] {
            b'+' => (true, &token[1..]),
            b'-' => (false, &token[1..]),
            _ => (true, token),
        };
        if rest.is_empty() {
            return Err(format!("empty label name in `{token}`"));
        }
        if rest
            .chars()
            .any(|c| !(c.is_alphanumeric() || c == '-' || c == '_' || c == '/'))
        {
            return Err(format!(
                "invalid label `{rest}`: only alphanumerics, `-`, `_`, `/` allowed"
            ));
        }
        Ok(if kind {
            LabelEdit::Add(rest.to_string())
        } else {
            LabelEdit::Remove(rest.to_string())
        })
    }
}

/// True if `s` is a well-formed todo id: exactly `ID_LEN` lowercase hex chars.
///
/// Why: ids flow into shell completion output (`git-todo complete ids`), where
/// bash's `compgen -W` would re-expand a malicious id like `aa$(...)` and run
/// arbitrary code. Keeping ids strictly hex blocks that at the boundary.
pub fn is_valid_id(s: &str) -> bool {
    s.len() == ID_LEN && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

// Generous ceilings on loaded field sizes. They exist to bound memory for a
// crafted commit (e.g. a 1 GB title), not to enforce product limits.
const MAX_SINGLE_LINE: usize = 1024;
const MAX_BODY: usize = 64 * 1024;

/// Validate a todo deserialized from the todo branch (an untrusted source).
///
/// Rejects:
/// - ids that aren't 8 lowercase hex chars (shell-completion injection)
/// - control characters in any field (terminal-escape injection when printed;
///   newlines in single-line fields also corrupt commit messages built from
///   them, e.g. `comment: {id} by {author}`)
/// - fields exceeding `MAX_SINGLE_LINE` / `MAX_BODY`
pub fn validate_loaded(todo: &Todo) -> Result<(), String> {
    if !is_valid_id(&todo.id) {
        return Err(format!(
            "invalid id `{}` (expected 8 lowercase hex chars)",
            todo.id
        ));
    }
    check_field("title", &todo.title, false)?;
    check_field("created_by", &todo.created_by, false)?;
    if let Some(by) = todo.done_by.as_deref() {
        check_field("done_by", by, false)?;
    }
    check_field("body", &todo.body, true)?;
    for l in &todo.labels {
        check_field("label", l, false)?;
    }
    for c in &todo.comments {
        check_field("comment.by", &c.by, false)?;
        check_field("comment.body", &c.body, true)?;
    }
    Ok(())
}

fn check_field(name: &str, s: &str, multiline: bool) -> Result<(), String> {
    let max = if multiline { MAX_BODY } else { MAX_SINGLE_LINE };
    if s.len() > max {
        return Err(format!("{name} exceeds {max} bytes"));
    }
    if let Some(bad) = s.chars().find(|&c| is_forbidden(c, multiline)) {
        return Err(format!(
            "{name} contains forbidden control char U+{:04X}",
            bad as u32
        ));
    }
    Ok(())
}

fn is_forbidden(c: char, allow_newline: bool) -> bool {
    match c {
        '\t' => false,
        '\n' if allow_newline => false,
        c if (c as u32) < 0x20 => true,
        '\u{7F}' => true,
        c if (0x80..=0x9F).contains(&(c as u32)) => true,
        _ => false,
    }
}

fn random_id() -> String {
    // fastrand seeds per-thread from the OS, so two processes adding todos
    // simultaneously draw from independent streams. Not crypto-grade, just
    // enough variation that natural collisions are rare; the caller still
    // checks `taken` and re-rolls if needed.
    let mut bytes = [0u8; ID_LEN / 2];
    fastrand::fill(&mut bytes);
    hex::encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed_time() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-04-19T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    /// Convenience: build a todo with an empty taken set.
    fn fresh(title: &str, body: &str, author: &str) -> Todo {
        Todo::new(
            title.into(),
            body.into(),
            author.into(),
            fixed_time(),
            &HashSet::new(),
        )
    }

    #[test]
    fn id_is_eight_chars_hex() {
        let t = fresh("Buy milk", "", "Jeremy <j@example.com>");
        assert_eq!(t.id.len(), ID_LEN);
        assert!(t.id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn toml_roundtrip_with_labels_and_comments() {
        let mut t = fresh("Buy milk", "two litres", "Jeremy <j@example.com>");
        t.apply_label_edits(&[
            LabelEdit::Add("chore".into()),
            LabelEdit::Add("shop".into()),
        ]);
        t.add_comment("Alice <a@x>".into(), "Whole or skim?".into(), fixed_time());
        t.add_comment("Jeremy <j@x>".into(), "Whole.".into(), fixed_time());
        t.mark_done("Jeremy <j@x>".into(), fixed_time());

        let s = t.to_toml().unwrap();
        let back = Todo::from_toml(&s).unwrap();
        assert_eq!(back.id, t.id);
        assert_eq!(back.labels, vec!["chore", "shop"]);
        assert_eq!(back.comments.len(), 2);
        assert_eq!(back.comments[0].body, "Whole or skim?");
        assert_eq!(back.status, Status::Done);
    }

    #[test]
    fn open_todo_omits_done_and_empty_collections() {
        let t = fresh("X", "", "A");
        let s = t.to_toml().unwrap();
        assert!(!s.contains("done ="));
        assert!(!s.contains("done_by"));
        assert!(!s.contains("labels"));
        assert!(!s.contains("[[comments]]"));
    }

    #[test]
    fn label_edits_idempotent() {
        let mut t = fresh("X", "", "A");
        let (added, removed) = t.apply_label_edits(&[
            LabelEdit::Add("a".into()),
            LabelEdit::Add("a".into()),
            LabelEdit::Remove("missing".into()),
        ]);
        assert_eq!(added, vec!["a"]);
        assert!(removed.is_empty());
        assert_eq!(t.labels, vec!["a"]);

        let (added, removed) = t.apply_label_edits(&[LabelEdit::Remove("a".into())]);
        assert!(added.is_empty());
        assert_eq!(removed, vec!["a"]);
        assert!(t.labels.is_empty());
    }

    #[test]
    fn is_valid_id_accepts_lowercase_hex_only() {
        assert!(is_valid_id("0123abcd"));
        assert!(is_valid_id("cafef00d"));
        // wrong length
        assert!(!is_valid_id(""));
        assert!(!is_valid_id("abc"));
        assert!(!is_valid_id("0123abcde"));
        // uppercase hex rejected (generate_id emits lowercase)
        assert!(!is_valid_id("CAFEF00D"));
        // shell-metachar payload from the exploit scenario
        assert!(!is_valid_id("aa$(touch"));
        assert!(!is_valid_id("aa\nbbccdd"));
    }

    #[test]
    fn label_edit_parse() {
        assert_eq!(LabelEdit::parse("+foo"), Ok(LabelEdit::Add("foo".into())));
        assert_eq!(LabelEdit::parse("-bar"), Ok(LabelEdit::Remove("bar".into())));
        assert_eq!(LabelEdit::parse("baz"), Ok(LabelEdit::Add("baz".into())));
        assert_eq!(
            LabelEdit::parse("kind/bug"),
            Ok(LabelEdit::Add("kind/bug".into()))
        );
        assert!(LabelEdit::parse("").is_err());
        assert!(LabelEdit::parse("+").is_err());
        assert!(LabelEdit::parse("+with space").is_err());
    }

    fn good_todo() -> Todo {
        fresh("Buy milk", "two litres", "Jeremy <j@example.com>")
    }

    #[test]
    fn validate_loaded_accepts_freshly_created_todo() {
        assert!(validate_loaded(&good_todo()).is_ok());
    }

    #[test]
    fn validate_loaded_rejects_escape_in_title() {
        let mut t = good_todo();
        // ANSI CSI (clear screen) — would execute in terminal if printed raw.
        t.title = "pwn\x1b[2J".into();
        assert!(validate_loaded(&t).is_err());
    }

    #[test]
    fn validate_loaded_rejects_newline_in_author() {
        let mut t = good_todo();
        t.created_by = "Alice\nInjected-Header: x".into();
        assert!(validate_loaded(&t).is_err());
    }

    #[test]
    fn validate_loaded_allows_newlines_in_body() {
        let mut t = good_todo();
        t.body = "line one\nline two\n".into();
        assert!(validate_loaded(&t).is_ok());
    }

    #[test]
    fn validate_loaded_rejects_oversized_title() {
        let mut t = good_todo();
        t.title = "a".repeat(MAX_SINGLE_LINE + 1);
        assert!(validate_loaded(&t).is_err());
    }

    #[test]
    fn validate_loaded_rejects_c1_control_in_body() {
        let mut t = good_todo();
        // U+0085 NEL — a C1 control.
        t.body = "oops\u{85}here".into();
        assert!(validate_loaded(&t).is_err());
    }

    #[test]
    fn validate_loaded_rejects_del_in_comment_author() {
        let mut t = good_todo();
        t.add_comment("Alice\x7F".into(), "hi".into(), fixed_time());
        assert!(validate_loaded(&t).is_err());
    }

    #[test]
    fn new_re_rolls_when_first_draw_is_taken() {
        // Seed deterministically, draw an id, then seed again with the same
        // value but mark that id taken. The next draw will produce the same
        // first id, see it's taken, and re-roll — so the resulting id must
        // differ from `first_id`.
        fastrand::seed(0xC0FFEE);
        let first_id = random_id();

        fastrand::seed(0xC0FFEE);
        let mut taken = HashSet::new();
        taken.insert(first_id.as_str());
        let t = Todo::new(
            "x".into(),
            String::new(),
            "a".into(),
            fixed_time(),
            &taken,
        );
        assert_ne!(t.id, first_id);
        assert_eq!(t.id.len(), ID_LEN);
        assert!(t.id.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
