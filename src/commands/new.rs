use anyhow::{Result, bail};

use crate::editor;
use crate::store::Store;

pub fn run(
    title: Option<String>,
    description: Option<String>,
    title_words: Vec<String>,
) -> Result<()> {
    let title_arg = title.or_else(|| {
        if title_words.is_empty() {
            None
        } else {
            Some(title_words.join(" "))
        }
    });

    let (title, body) = match (title_arg, description) {
        (Some(t), Some(d)) => (t, d),
        (Some(t), None) => (t, String::new()),
        (None, _) => {
            let composed = editor::compose_new()?;
            if composed.title.is_empty() {
                bail!("aborting: empty title");
            }
            (composed.title, composed.body)
        }
    };

    let mut store = Store::open()?;
    let todo = store.add(title, body)?;
    println!("{}  {}", todo.id, todo.title);
    Ok(())
}
