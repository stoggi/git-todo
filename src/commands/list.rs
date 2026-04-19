use anyhow::Result;
use chrono::Utc;

use crate::store::Store;
use crate::todo::{Status, Todo};

pub enum Filter {
    Open,
    Done,
    All,
}

pub fn run(filter: Filter) -> Result<()> {
    let store = Store::open()?;
    let now = Utc::now();
    let todos: Vec<&Todo> = store
        .todos()
        .iter()
        .filter(|t| match filter {
            Filter::Open => t.is_open(),
            Filter::Done => matches!(t.status, Status::Done),
            Filter::All => true,
        })
        .collect();

    if todos.is_empty() {
        return Ok(());
    }

    let title_width = todos
        .iter()
        .map(|t| t.title.chars().count())
        .max()
        .unwrap_or(0)
        .min(60);

    for t in todos {
        let mark = if t.is_open() { "[ ]" } else { "[x]" };
        let age = humanize_age(now.signed_duration_since(t.created));
        let title = truncate(&t.title, title_width);
        println!(
            "{id}  {mark}  {title:<width$}  {age:>6}  {author}",
            id = t.id,
            mark = mark,
            title = title,
            width = title_width,
            age = age,
            author = t.created_by,
        );
    }
    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn humanize_age(d: chrono::Duration) -> String {
    let secs = d.num_seconds().max(0);
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86400 {
        format!("{}h", secs / 3600)
    } else if secs < 86400 * 30 {
        format!("{}d", secs / 86400)
    } else if secs < 86400 * 365 {
        format!("{}mo", secs / (86400 * 30))
    } else {
        format!("{}y", secs / (86400 * 365))
    }
}
