use anyhow::{Result, anyhow, bail};
use chrono::Utc;

use crate::repo::Repo;
use crate::todo::{LabelEdit, Todo};

pub struct Store {
    repo: Repo,
    todos: Vec<Todo>,
}

impl Store {
    pub fn open() -> Result<Self> {
        let repo = Repo::discover()?;
        let todos = repo.load_todos()?;
        Ok(Self { repo, todos })
    }

    pub fn todos(&self) -> &[Todo] {
        &self.todos
    }

    pub fn find(&self, prefix: &str) -> Result<&Todo> {
        let idx = self.find_index(prefix, false)?;
        Ok(&self.todos[idx])
    }

    pub fn add(&mut self, title: String, body: String) -> Result<&Todo> {
        let author = self.repo.identity_string()?;
        let todo = Todo::new(title, body, author, Utc::now());
        let message = format!("new: {} ({})", todo.title, todo.id);
        self.todos.push(todo);
        self.repo.commit_snapshot(&message, &self.todos)?;
        Ok(self.todos.last().unwrap())
    }

    pub fn mark_done(&mut self, prefix: &str) -> Result<&Todo> {
        let idx = self.find_index(prefix, true)?;
        let author = self.repo.identity_string()?;
        self.todos[idx].mark_done(author, Utc::now());
        let id = self.todos[idx].id.clone();
        let message = format!("done: {id}");
        self.repo.commit_snapshot(&message, &self.todos)?;
        Ok(&self.todos[idx])
    }

    pub fn edit_labels(&mut self, prefix: &str, edits: &[LabelEdit]) -> Result<&Todo> {
        let idx = self.find_index(prefix, false)?;
        let (added, removed) = self.todos[idx].apply_label_edits(edits);
        if added.is_empty() && removed.is_empty() {
            bail!("no label changes to apply");
        }
        let id = self.todos[idx].id.clone();
        let mut summary = String::new();
        for a in &added {
            summary.push_str(&format!(" +{a}"));
        }
        for r in &removed {
            summary.push_str(&format!(" -{r}"));
        }
        let message = format!("label: {id}{summary}");
        self.repo.commit_snapshot(&message, &self.todos)?;
        Ok(&self.todos[idx])
    }

    pub fn add_comment(&mut self, prefix: &str, body: String) -> Result<&Todo> {
        if body.trim().is_empty() {
            bail!("aborting: empty comment");
        }
        let idx = self.find_index(prefix, false)?;
        let author = self.repo.identity_string()?;
        self.todos[idx].add_comment(author.clone(), body, Utc::now());
        let id = self.todos[idx].id.clone();
        let message = format!("comment: {id} by {author}");
        self.repo.commit_snapshot(&message, &self.todos)?;
        Ok(&self.todos[idx])
    }

    /// Look up a todo by id prefix. If `open_only`, restrict the candidate set
    /// to open todos (used by `done` so a finished todo can't be re-finished).
    fn find_index(&self, prefix: &str, open_only: bool) -> Result<usize> {
        if prefix.is_empty() {
            bail!("empty id");
        }
        let matches: Vec<usize> = self
            .todos
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
                let ids: Vec<&str> = matches
                    .iter()
                    .map(|&i| self.todos[i].id.as_str())
                    .collect();
                Err(anyhow!(
                    "ambiguous id `{prefix}` matches {n} todos: {}",
                    ids.join(", ")
                ))
            }
        }
    }
}
