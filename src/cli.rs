use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "git-todo",
    bin_name = "git todo",
    version,
    about = "Track todos as commits on a `todo` branch",
    disable_help_subcommand = true
)]
pub struct Cli {
    /// Write the man page (roff source) to stdout and exit.
    #[arg(long, global = false)]
    pub generate_man: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Create a new todo. With no title, opens $EDITOR.
    New {
        /// Title for the todo.
        #[arg(short = 't', long = "title")]
        title: Option<String>,
        /// Description / body text.
        #[arg(short = 'd', long = "description")]
        description: Option<String>,
        /// Title as positional words (convenience; conflicts with -t).
        #[arg(conflicts_with = "title")]
        title_words: Vec<String>,
    },
    /// Mark a todo as done.
    Done {
        /// Todo id (or unambiguous prefix).
        id: String,
    },
    /// List todos (default: open only).
    List {
        /// Show done todos as well.
        #[arg(long)]
        all: bool,
        /// Show only done todos.
        #[arg(long, conflicts_with = "all")]
        done: bool,
    },
    /// Show full details for a todo.
    Show {
        /// Todo id (or unambiguous prefix).
        id: String,
    },
    /// Add (`+name`) or remove (`-name`) labels.
    Label {
        /// Todo id (or unambiguous prefix).
        id: String,
        /// Label edits, e.g. `+chore -urgent`. Bare `name` adds.
        #[arg(allow_hyphen_values = true)]
        edits: Vec<String>,
    },
    /// Add a comment. With no `-m`, opens $EDITOR.
    Comment {
        /// Todo id (or unambiguous prefix).
        id: String,
        /// Comment body. Omit to open $EDITOR.
        #[arg(short, long)]
        message: Option<String>,
    },
}
