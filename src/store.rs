use std::collections::HashSet;
#[cfg(test)]
use std::path::Path;

use anyhow::{Result, anyhow, bail};
use chrono::Utc;
use git2::Oid;

use crate::repo::{Repo, is_cas_conflict};
use crate::todo::{LabelEdit, Todo};

/// How many times to reload-and-retry on a lost CAS race before giving up.
const MAX_ATTEMPTS: usize = 5;

pub struct Store {
    repo: Repo,
    todos: Vec<Todo>,
    parent: Option<Oid>,
}

impl Store {
    pub fn open() -> Result<Self> {
        Self::open_repo(Repo::discover()?)
    }

    #[cfg(test)]
    pub fn open_at(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_repo(Repo::discover_at(path)?)
    }

    fn open_repo(repo: Repo) -> Result<Self> {
        let parent = repo.todo_tip()?;
        let todos = repo.load_todos()?;
        Ok(Self {
            repo,
            todos,
            parent,
        })
    }

    pub fn todos(&self) -> &[Todo] {
        &self.todos
    }

    pub fn find(&self, prefix: &str) -> Result<&Todo> {
        let idx = find_index(&self.todos, prefix, false)?;
        Ok(&self.todos[idx])
    }

    pub fn add(&mut self, title: String, body: String) -> Result<&Todo> {
        let author = self.repo.identity_string()?;
        let now = Utc::now();
        let idx = self.commit_with_retry(&mut |todos| {
            // Re-derived per attempt: on a CAS retry the reloaded `todos` may
            // contain a racing-twin id that we'd collide with at salt 0.
            let new_todo = {
                let taken: HashSet<&str> = todos.iter().map(|t| t.id.as_str()).collect();
                Todo::new(title.clone(), body.clone(), author.clone(), now, &taken)
            };
            let message = format!("new: {} ({})", new_todo.title, new_todo.id);
            todos.push(new_todo);
            Ok((todos.len() - 1, message))
        })?;
        Ok(&self.todos[idx])
    }

    pub fn mark_done(&mut self, prefix: &str) -> Result<&Todo> {
        let author = self.repo.identity_string()?;
        let now = Utc::now();
        let prefix = prefix.to_string();
        let idx = self.commit_with_retry(&mut |todos| {
            let idx = find_index(todos, &prefix, true)?;
            todos[idx].mark_done(author.clone(), now);
            let id = todos[idx].id.clone();
            Ok((idx, format!("done: {id}")))
        })?;
        Ok(&self.todos[idx])
    }

    pub fn edit_labels(&mut self, prefix: &str, edits: &[LabelEdit]) -> Result<&Todo> {
        let prefix = prefix.to_string();
        let edits = edits.to_vec();
        let idx = self.commit_with_retry(&mut |todos| {
            let idx = find_index(todos, &prefix, false)?;
            let (added, removed) = todos[idx].apply_label_edits(&edits);
            if added.is_empty() && removed.is_empty() {
                bail!("no label changes to apply");
            }
            let id = todos[idx].id.clone();
            let mut summary = String::new();
            for a in &added {
                summary.push_str(&format!(" +{a}"));
            }
            for r in &removed {
                summary.push_str(&format!(" -{r}"));
            }
            Ok((idx, format!("label: {id}{summary}")))
        })?;
        Ok(&self.todos[idx])
    }

    pub fn add_comment(&mut self, prefix: &str, body: String) -> Result<&Todo> {
        if body.trim().is_empty() {
            bail!("aborting: empty comment");
        }
        let author = self.repo.identity_string()?;
        let now = Utc::now();
        let prefix = prefix.to_string();
        let idx = self.commit_with_retry(&mut |todos| {
            let idx = find_index(todos, &prefix, false)?;
            todos[idx].add_comment(author.clone(), body.clone(), now);
            let id = todos[idx].id.clone();
            Ok((idx, format!("comment: {id} by {author}")))
        })?;
        Ok(&self.todos[idx])
    }

    /// Apply `op` against the in-memory todos and commit the result. On a CAS
    /// conflict, reload from the (now-moved) ref and re-run `op` so the change
    /// is layered onto whatever the racing writer added. The op returns the
    /// affected todo's index and the commit message it wants used.
    fn commit_with_retry(
        &mut self,
        op: &mut dyn FnMut(&mut Vec<Todo>) -> Result<(usize, String)>,
    ) -> Result<usize> {
        for _ in 0..MAX_ATTEMPTS {
            let (idx, message) = op(&mut self.todos)?;
            match self
                .repo
                .commit_snapshot(self.parent, &message, &self.todos)
            {
                Ok(new_oid) => {
                    self.parent = Some(new_oid);
                    return Ok(idx);
                }
                Err(e) if is_cas_conflict(&e) => {
                    self.parent = self.repo.todo_tip()?;
                    self.todos = self.repo.load_todos()?;
                }
                Err(e) => return Err(e),
            }
        }
        Err(anyhow!(
            "todo branch kept moving under us — gave up after {MAX_ATTEMPTS} attempts"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Repository;
    use tempfile::TempDir;

    fn init_repo() -> TempDir {
        let dir = TempDir::new().unwrap();
        let inner = Repository::init(dir.path()).unwrap();
        let mut cfg = inner.config().unwrap();
        cfg.set_str("user.name", "Test").unwrap();
        cfg.set_str("user.email", "test@example.com").unwrap();
        dir
    }

    /// Simulate a racing writer: open a second Store against the same repo,
    /// add `title`, and drop it. The original store now has a stale parent
    /// OID and a stale todos vec.
    fn racing_add(path: &Path, title: &str) {
        let mut other = Store::open_at(path).unwrap();
        other.add(title.into(), String::new()).unwrap();
    }

    #[test]
    fn add_retries_after_concurrent_add_and_keeps_both() {
        let dir = init_repo();
        let mut store = Store::open_at(dir.path()).unwrap();
        store.add("first".into(), String::new()).unwrap();

        // Another process commits between our load and our next write.
        racing_add(dir.path(), "racing");

        store.add("ours".into(), String::new()).unwrap();

        // Reopen fresh and verify all three are present — the racing add
        // would have been silently dropped without CAS+retry.
        let fresh = Store::open_at(dir.path()).unwrap();
        let titles: Vec<&str> = fresh.todos().iter().map(|t| t.title.as_str()).collect();
        assert!(titles.contains(&"first"), "missing `first`: {titles:?}");
        assert!(titles.contains(&"racing"), "missing `racing`: {titles:?}");
        assert!(titles.contains(&"ours"), "missing `ours`: {titles:?}");
    }

    #[test]
    fn done_retries_after_concurrent_add() {
        let dir = init_repo();
        let mut store = Store::open_at(dir.path()).unwrap();
        let id = store.add("target".into(), String::new()).unwrap().id.clone();

        racing_add(dir.path(), "noise");

        store.mark_done(&id).unwrap();

        let fresh = Store::open_at(dir.path()).unwrap();
        let target = fresh.todos().iter().find(|t| t.id == id).unwrap();
        assert!(!target.is_open());
        assert!(fresh.todos().iter().any(|t| t.title == "noise"));
    }
}

/// Look up a todo by id prefix. If `open_only`, restrict the candidate set
/// to open todos (used by `done` so a finished todo can't be re-finished).
fn find_index(todos: &[Todo], prefix: &str, open_only: bool) -> Result<usize> {
    if prefix.is_empty() {
        bail!("empty id");
    }
    let matches: Vec<usize> = todos
        .iter()
        .enumerate()
        .filter(|(_, t)| (!open_only || t.is_open()) && t.id.starts_with(prefix))
        .map(|(i, _)| i)
        .collect();
    match matches.len() {
        0 => {
            let scope = if open_only { "open " } else { "" };
            Err(anyhow!("no {scope}todo matches id `{prefix}`"))
        }
        1 => Ok(matches[0]),
        n => {
            let ids: Vec<&str> = matches.iter().map(|&i| todos[i].id.as_str()).collect();
            Err(anyhow!(
                "ambiguous id `{prefix}` matches {n} todos: {}",
                ids.join(", ")
            ))
        }
    }
}
