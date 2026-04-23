use std::path::Path;

use anyhow::{Context, Result, anyhow};
use git2::{ErrorCode, ObjectType, Oid, Repository, Signature};

use crate::todo::{Todo, validate_loaded};

const TODO_REF: &str = "refs/heads/todo";
const TODOS_DIR: &str = "todos";
const FILE_MODE: i32 = 0o100644;
const TREE_MODE: i32 = 0o040000;

/// True if `err` indicates a lost CAS race on the todo ref — either the ref
/// moved (`Modified`) or it appeared since we last looked (`Exists` from the
/// `force=false` create path used when expected_parent is None).
pub fn is_cas_conflict(err: &anyhow::Error) -> bool {
    err.downcast_ref::<git2::Error>()
        .map(|e| matches!(e.code(), ErrorCode::Modified | ErrorCode::Exists))
        .unwrap_or(false)
}

pub struct Repo {
    inner: Repository,
}

impl Repo {
    pub fn discover() -> Result<Self> {
        Self::discover_at(".")
    }

    pub fn discover_at(path: impl AsRef<Path>) -> Result<Self> {
        let inner = Repository::discover(path)
            .context("not in a git repository (and no parent directory is one)")?;
        Ok(Self { inner })
    }

    pub fn identity_string(&self) -> Result<String> {
        let cfg = self.inner.config().context("reading git config")?;
        let name = cfg
            .get_string("user.name")
            .context("git config user.name is not set")?;
        let email = cfg
            .get_string("user.email")
            .context("git config user.email is not set")?;
        Ok(format!("{name} <{email}>"))
    }

    fn signature(&self) -> Result<Signature<'static>> {
        let cfg = self.inner.config()?;
        let name = cfg
            .get_string("user.name")
            .context("git config user.name is not set")?;
        let email = cfg
            .get_string("user.email")
            .context("git config user.email is not set")?;
        Ok(Signature::now(&name, &email)?)
    }

    pub fn todo_tip(&self) -> Result<Option<Oid>> {
        match self.inner.refname_to_id(TODO_REF) {
            Ok(oid) => Ok(Some(oid)),
            Err(e) if e.code() == ErrorCode::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn load_todos(&self) -> Result<Vec<Todo>> {
        let Some(tip) = self.todo_tip()? else {
            return Ok(Vec::new());
        };
        let commit = self.inner.find_commit(tip)?;
        let root = commit.tree()?;
        let todos_entry = match root.get_name(TODOS_DIR) {
            Some(e) => e,
            None => return Ok(Vec::new()),
        };
        let todos_obj = todos_entry.to_object(&self.inner)?;
        let todos_tree = todos_obj
            .as_tree()
            .ok_or_else(|| anyhow!("`{TODOS_DIR}` in todo branch is not a tree"))?;

        let mut out = Vec::with_capacity(todos_tree.len());
        for entry in todos_tree.iter() {
            if entry.kind() != Some(ObjectType::Blob) {
                continue;
            }
            let name = entry.name().unwrap_or("");
            if !name.ends_with(".toml") {
                continue;
            }
            let blob = entry.to_object(&self.inner)?;
            let blob = blob
                .as_blob()
                .ok_or_else(|| anyhow!("entry {name} is not a blob"))?;
            let s = std::str::from_utf8(blob.content())
                .with_context(|| format!("todo file {name} is not valid UTF-8"))?;
            let todo = Todo::from_toml(s)
                .with_context(|| format!("parsing todo file {name}"))?;
            validate_loaded(&todo)
                .map_err(|e| anyhow!("rejecting todo file {name}: {e}"))?;
            if name != format!("{}.toml", todo.id) {
                return Err(anyhow!(
                    "todo file {name} does not match its id `{}`",
                    todo.id
                ));
            }
            out.push(todo);
        }
        out.sort_by(|a, b| a.created.cmp(&b.created));
        Ok(out)
    }

    /// Commit `todos` as a snapshot. The new commit's parent is
    /// `expected_parent`, and the ref move is conditional on the ref still
    /// pointing at `expected_parent` — so a racing writer can't be silently
    /// clobbered. On a lost race, returns an error for which `is_cas_conflict`
    /// is true; the caller is expected to reload and retry.
    pub fn commit_snapshot(
        &self,
        expected_parent: Option<Oid>,
        message: &str,
        todos: &[Todo],
    ) -> Result<Oid> {
        let mut todos_tb = self.inner.treebuilder(None)?;
        for todo in todos {
            let bytes = todo.to_toml()?.into_bytes();
            let blob_oid = self.inner.blob(&bytes)?;
            todos_tb.insert(format!("{}.toml", todo.id), blob_oid, FILE_MODE)?;
        }
        let todos_tree_oid = todos_tb.write()?;

        let mut root_tb = self.inner.treebuilder(None)?;
        root_tb.insert(TODOS_DIR, todos_tree_oid, TREE_MODE)?;
        let root_tree_oid = root_tb.write()?;
        let root_tree = self.inner.find_tree(root_tree_oid)?;

        let sig = self.signature()?;
        let parents_owned: Vec<git2::Commit> = match expected_parent {
            Some(oid) => vec![self.inner.find_commit(oid)?],
            None => vec![],
        };
        let parent_refs: Vec<&git2::Commit> = parents_owned.iter().collect();

        // Build the commit object without touching any ref. If the CAS below
        // fails, this commit becomes orphaned and is reclaimed by `git gc`.
        let oid = self
            .inner
            .commit(None, &sig, &sig, message, &root_tree, &parent_refs)?;

        match expected_parent {
            Some(parent) => {
                self.inner
                    .reference_matching(TODO_REF, oid, true, parent, message)?;
            }
            None => {
                // No prior tip — succeed only if the ref still doesn't exist.
                self.inner.reference(TODO_REF, oid, false, message)?;
            }
        }
        Ok(oid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use tempfile::TempDir;

    fn init_repo() -> (TempDir, Repo) {
        let dir = TempDir::new().unwrap();
        let inner = Repository::init(dir.path()).unwrap();
        let mut cfg = inner.config().unwrap();
        cfg.set_str("user.name", "Test").unwrap();
        cfg.set_str("user.email", "test@example.com").unwrap();
        drop(inner);
        let repo = Repo::discover_at(dir.path()).unwrap();
        (dir, repo)
    }

    fn fresh_todo(title: &str) -> Todo {
        Todo::new(
            title.into(),
            String::new(),
            "Test <test@example.com>".into(),
            Utc::now(),
        )
    }

    #[test]
    fn first_commit_succeeds_with_no_parent() {
        let (_dir, repo) = init_repo();
        let oid = repo
            .commit_snapshot(None, "init", &[fresh_todo("a")])
            .unwrap();
        assert_eq!(repo.todo_tip().unwrap(), Some(oid));
    }

    #[test]
    fn second_commit_with_no_parent_is_cas_conflict() {
        let (_dir, repo) = init_repo();
        repo.commit_snapshot(None, "init", &[fresh_todo("a")])
            .unwrap();
        // Pretending the ref doesn't exist when it does should be rejected.
        let err = repo
            .commit_snapshot(None, "init again", &[fresh_todo("b")])
            .unwrap_err();
        assert!(is_cas_conflict(&err), "expected CAS conflict, got: {err:?}");
    }

    #[test]
    fn stale_parent_is_cas_conflict() {
        let (_dir, repo) = init_repo();
        let first = repo
            .commit_snapshot(None, "init", &[fresh_todo("a")])
            .unwrap();
        let second = repo
            .commit_snapshot(Some(first), "second", &[fresh_todo("b")])
            .unwrap();
        // Try to commit again pointing at the original tip; the ref now
        // points at `second`, so this must be rejected.
        let err = repo
            .commit_snapshot(Some(first), "stale", &[fresh_todo("c")])
            .unwrap_err();
        assert!(is_cas_conflict(&err), "expected CAS conflict, got: {err:?}");
        assert_eq!(repo.todo_tip().unwrap(), Some(second));
    }

    #[test]
    fn correct_parent_succeeds() {
        let (_dir, repo) = init_repo();
        let first = repo
            .commit_snapshot(None, "init", &[fresh_todo("a")])
            .unwrap();
        let second = repo
            .commit_snapshot(Some(first), "second", &[fresh_todo("b")])
            .unwrap();
        assert_eq!(repo.todo_tip().unwrap(), Some(second));
    }
}
