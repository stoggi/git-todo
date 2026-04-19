mod cli;
mod commands;
mod editor;
mod repo;
mod store;
mod todo;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser};

use cli::{Cli, Command};
use commands::list::Filter;

fn main() {
    if let Err(e) = run() {
        eprintln!("git-todo: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    if cli.generate_man {
        let cmd = Cli::command();
        let man = clap_mangen::Man::new(cmd);
        let mut out = std::io::stdout().lock();
        man.render(&mut out).context("rendering man page")?;
        return Ok(());
    }

    match cli.command {
        None => commands::list::run(Filter::Open),
        Some(Command::List { all, done }) => {
            let f = if all {
                Filter::All
            } else if done {
                Filter::Done
            } else {
                Filter::Open
            };
            commands::list::run(f)
        }
        Some(Command::New {
            title,
            description,
            title_words,
        }) => commands::new::run(title, description, title_words),
        Some(Command::Done { id }) => commands::done::run(id),
        Some(Command::Show { id }) => commands::show::run(id),
        Some(Command::Label { id, edits }) => commands::label::run(id, edits),
        Some(Command::Comment { id, message }) => commands::comment::run(id, message),
    }
}
