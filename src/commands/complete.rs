use anyhow::Result;

use crate::store::Store;
use crate::todo::Status;

pub fn ids(open: bool, all: bool) -> Result<()> {
    // Default when neither flag is given: all todos. The `all` flag is a
    // noisy-but-explicit opt-in that lines up with list's `--all`.
    let open_only = open && !all;
    let store = Store::open()?;
    for t in store.todos() {
        if open_only && !matches!(t.status, Status::Open) {
            continue;
        }
        println!("{}", t.id);
    }
    Ok(())
}
