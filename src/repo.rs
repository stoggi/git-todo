use anyhow::{Context, Result, anyhow};
use git2::{ErrorCode, ObjectType, Oid, Repository, Signature};

use crate::todo::{Todo, validate_loaded};

const TODO_REF: &str = "refs/heads/todo";
const TODOS_DIR: &str = "todos";
const FILE_MODE: i32 = 0o100644;
const TREE_MODE: i32 = 0o040000;

pub struct Repo {
    inner: Repository,
}

impl Repo {
    pub fn discover() -> Result<Self> {
        let inner = Repository::discover(".")
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

    fn todo_tip(&self) -> Result<Option<Oid>> {
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

    pub fn commit_snapshot(&self, message: &str, todos: &[Todo]) -> Result<Oid> {
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
        let parent_oid = self.todo_tip()?;
        let parents_owned: Vec<git2::Commit> = match parent_oid {
            Some(oid) => vec![self.inner.find_commit(oid)?],
            None => vec![],
        };
        let parent_refs: Vec<&git2::Commit> = parents_owned.iter().collect();

        let oid = self.inner.commit(
            Some(TODO_REF),
            &sig,
            &sig,
            message,
            &root_tree,
            &parent_refs,
        )?;
        Ok(oid)
    }
}
