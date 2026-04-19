use anyhow::{Result, bail};

use crate::editor;
use crate::store::Store;

pub fn run(id: String, message: Option<String>) -> Result<()> {
    let body = match message {
        Some(m) => m,
        None => editor::compose_comment()?,
    };
    if body.trim().is_empty() {
        bail!("aborting: empty comment");
    }
    let mut store = Store::open()?;
    let todo = store.add_comment(&id, body)?;
    println!("{}  +1 comment ({} total)", todo.id, todo.comments.len());
    Ok(())
}
