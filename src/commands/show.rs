use anyhow::Result;

use crate::store::Store;
use crate::todo::Status;

pub fn run(id: String) -> Result<()> {
    let store = Store::open()?;
    let todo = store.find(&id)?;

    let status = match todo.status {
        Status::Open => "open",
        Status::Done => "done",
    };
    println!("id:      {}", todo.id);
    println!("title:   {}", todo.title);
    println!("status:  {status}");
    println!("created: {}  by {}", todo.created.to_rfc3339(), todo.created_by);
    if let (Some(at), Some(by)) = (todo.done.as_ref(), todo.done_by.as_ref()) {
        println!("done:    {}  by {by}", at.to_rfc3339());
    }
    if !todo.labels.is_empty() {
        println!("labels:  {}", todo.labels.join(", "));
    }
    if !todo.body.is_empty() {
        println!();
        for line in todo.body.lines() {
            println!("    {line}");
        }
    }
    if !todo.comments.is_empty() {
        for c in &todo.comments {
            println!();
            println!("--- {} by {}", c.at.to_rfc3339(), c.by);
            for line in c.body.lines() {
                println!("    {line}");
            }
        }
    }
    Ok(())
}
