use anyhow::Result;

use crate::store::Store;

pub fn run(id: String) -> Result<()> {
    let mut store = Store::open()?;
    let todo = store.mark_done(&id)?;
    println!("done: {}  {}", todo.id, todo.title);
    Ok(())
}
