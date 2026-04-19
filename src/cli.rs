use clap::{Parser, Subcommand};
use clap_complete::Shell;

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
    #[arg(long)]
    pub generate_man: bool,

    /// Write a shell completion script to stdout and exit.
    #[arg(long, value_name = "SHELL")]
    pub generate_completion: Option<Shell>,

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
    /// Internal: print completion candidates (used by shell completion scripts).
    #[command(hide = true)]
    Complete {
        #[command(subcommand)]
        what: CompleteWhat,
    },
}

#[derive(Subcommand, Debug)]
pub enum CompleteWhat {
    /// Print one todo id per line.
    Ids {
        /// Limit to open todos only.
        #[arg(long, conflicts_with = "all")]
        open: bool,
        /// Include both open and done (default).
        #[arg(long)]
        all: bool,
    },
}

pub fn completion_script(shell: Shell) -> Vec<u8> {
    use clap::CommandFactory;
    let mut cmd = Cli::command();
    let bin_name = "git-todo";
    let mut buf = Vec::new();
    clap_complete::generate(shell, &mut cmd, bin_name, &mut buf);
    match shell {
        Shell::Fish => buf.extend_from_slice(FISH_DYNAMIC),
        Shell::Bash => buf.extend_from_slice(BASH_DYNAMIC),
        Shell::Zsh => buf.extend_from_slice(ZSH_DYNAMIC),
        _ => {}
    }
    buf
}

// Fish: multiple `complete` registrations stack naturally. The static script
// adds plain argument completion; these lines add the dynamic id candidates
// whenever we're at the id positional of a hash-taking subcommand.
const FISH_DYNAMIC: &[u8] = b"

# Dynamic todo id completion (appended by git-todo --generate-completion).
complete -c git-todo -n '__fish_seen_subcommand_from done' -f -a '(command git-todo complete ids --open 2>/dev/null)'
complete -c git-todo -n '__fish_seen_subcommand_from show label comment' -f -a '(command git-todo complete ids --all 2>/dev/null)'
";

// Bash: wrap the generated `_git-todo` function so we can inject id candidates
// after the static completion has run. We swap the generated COMPREPLY when we
// detect that the current word is the id positional of a hash-taking command.
const BASH_DYNAMIC: &[u8] = b"

# Dynamic todo id completion (appended by git-todo --generate-completion).
_git-todo__dynamic() {
    local sub=\"${COMP_WORDS[1]}\"
    local flag
    case \"$sub\" in
        done) flag=\"--open\" ;;
        show|label|comment) flag=\"--all\" ;;
        *) return 1 ;;
    esac
    # Only the first positional (i.e. immediately after the subcommand)
    [ \"$COMP_CWORD\" = \"2\" ] || return 1
    local cur=\"${COMP_WORDS[COMP_CWORD]}\"
    local ids
    ids=$(command git-todo complete ids $flag 2>/dev/null) || return 1
    COMPREPLY=( $(compgen -W \"$ids\" -- \"$cur\") )
    return 0
}
_git-todo__wrapped() {
    _git-todo \"$@\" 2>/dev/null
    _git-todo__dynamic && return 0
}
complete -F _git-todo__wrapped -o bashdefault -o default git-todo
";

// Zsh: override the id-positional handler for the four hash-taking commands
// using _arguments-compatible custom completion. This appends beside the
// generated function and re-registers with compdef.
const ZSH_DYNAMIC: &[u8] = b"

# Dynamic todo id completion (appended by git-todo --generate-completion).
_git-todo__ids_open() {
    local -a ids
    ids=( ${(f)\"$(command git-todo complete ids --open 2>/dev/null)\"} )
    compadd -- $ids
}
_git-todo__ids_all() {
    local -a ids
    ids=( ${(f)\"$(command git-todo complete ids --all 2>/dev/null)\"} )
    compadd -- $ids
}
_git-todo__dispatch() {
    case \"$words[2]\" in
        done)
            if (( CURRENT == 3 )); then _git-todo__ids_open; return; fi ;;
        show|label|comment)
            if (( CURRENT == 3 )); then _git-todo__ids_all; return; fi ;;
    esac
    _git-todo \"$@\"
}
compdef _git-todo__dispatch git-todo
";
