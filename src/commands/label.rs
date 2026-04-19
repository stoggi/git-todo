use anyhow::{Context, Result, bail};

use crate::store::Store;
use crate::todo::LabelEdit;

pub fn run(id: String, edit_tokens: Vec<String>) -> Result<()> {
    if edit_tokens.is_empty() {
        bail!("no label edits given (try `+name` or `-name`)");
    }
    let edits: Vec<LabelEdit> = edit_tokens
        .iter()
        .map(|t| LabelEdit::parse(t).map_err(anyhow::Error::msg))
        .collect::<Result<_>>()
        .context("parsing label edits")?;

    let mut store = Store::open()?;
    let todo = store.edit_labels(&id, &edits)?;
    let labels = if todo.labels.is_empty() {
        "(none)".to_string()
    } else {
        todo.labels.join(", ")
    };
    println!("{}  labels: {labels}", todo.id);
    Ok(())
}
