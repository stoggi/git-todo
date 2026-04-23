# git-todo

> **Disclaimer:** this project was written in combination with an LLM
> ([Claude Code](https://claude.com/claude-code), Opus 4.7).
> Provided as-is, with no warranty, mileage may vary.

Track todos as commits on a `todo` branch. A tiny, opinionated git-native
todo tracker written in Rust.

Inspired by [git-bug](https://github.com/MichaelMure/git-bug), but deliberately
stripped down. git-bug's design is powerful (event-sourced DAG, Lamport
clocks, distributed identity). Overkill if all you want is a todo list that
lives next to your code. git-todo keeps the parts that matter and drops the
rest:

- **Todos live on a visible `todo` branch**, not hidden refs. `git log todo`,
  `git show todo:todos/<id>.toml`, and GitHub's branch view all just work.
- **Sync is plain `git push origin todo` / `git fetch origin todo`**
- **Identity comes from `git config user.name` / `user.email`** - no separate
  identity layer to manage.
- **Works as a git subcommand**: `git todo new`, `git todo done`, etc. the
  binary is named `git-todo` and git's built-in subcommand discovery does the
  rest.

## Install

### Using cargo

```sh
cargo install --git https://github.com/stoggi/git-todo
```

Installs `git-todo` to `~/.cargo/bin/`.

Shell completion (pick the line for your shell):

```sh
# fish
git todo --generate-completion fish > ~/.config/fish/completions/git-todo.fish

# zsh (ensure the dir is on $fpath)
git todo --generate-completion zsh  > ~/.zsh/completions/_git-todo

# bash
git todo --generate-completion bash > ~/.local/share/bash-completion/completions/git-todo
```

Completion handles subcommand and flag names statically; todo id arguments
on `done`, `show`, `label`, and `comment` are completed dynamically from the
current store.

Run `git todo -h` for help.

## Usage

```sh
# Create a todo
git todo new -t "Buy milk" -d "two litres from the corner shop"
git todo new Buy milk                  # positional title also works
git todo new                           # opens $EDITOR for title + body

# List
git todo                               # open todos
git todo list --all                    # open + done
git todo list --done                   # done only

# Mark done (short id prefixes work)
git todo done abc1

# Inspect
git todo show abc12345

# Labels
git todo label abc1 +chore -urgent     # add + / remove -
git todo label abc1 +shop

# Comments
git todo comment abc1 -m "whole or skim?"
git todo comment abc1                  # opens $EDITOR
```

`$EDITOR` is exec'd directly with no shell, so multi-word values like
`code -w` won't work. Point `$EDITOR` at a wrapper script if you need flags.

## Syncing across machines

The `todo` branch is a normal branch, so pushing is just:

```sh
git push origin todo
```

Pulling on another machine needs a little more care: plain `git pull` would
merge origin's current branch into whatever you have checked out. You want to
update the local `todo` branch directly (you never check it out). Use an
explicit refspec:

```sh
git fetch origin todo:todo          # fails if local and remote diverged
git fetch origin +todo:todo         # force local todo to match remote
```

To make plain `git fetch` always bring `todo` along, add it to the remote's
fetch refspecs once:

```sh
git config --add remote.origin.fetch '+refs/heads/todo:refs/heads/todo'
```

After that, `git fetch` on the second machine updates `todo` alongside your
normal branches, and `git todo` reflects the latest state.

**Divergence**: if you add todos on both machines before syncing, the forcing
refspec (`+todo:todo`) would overwrite local changes. The non-forcing form
fails and makes you notice. Resolving a diverged `todo` means a manual
`git merge` between the two tips.

## How it stores data

Every change is one commit on `refs/heads/todo`. The branch's tree is a full
snapshot of every todo as a TOML file:

```
todos/
  abc12345.toml
  def67890.toml
```

A todo looks like:

```toml
id = "abc12345"
title = "Buy milk"
status = "open"
created = 2026-04-19T10:00:00Z
created_by = "Jeremy Stott <jeremy@example.com>"
labels = ["chore", "shop"]
body = "two litres from the corner shop"

[[comments]]
at = 2026-04-19T11:00:00Z
by = "Alice <alice@example.com>"
body = "Whole or skim?"
```

Each operation becomes one commit whose message describes the action, so
`git log todo --oneline` reads as an activity log:

```
done: 74885aea
comment: 74885aea by Jeremy Stott <jeremy@example.com>
label: 74885aea +chore +shop
new: Buy milk (74885aea)
```

The `todo` branch is never checked out - git-todo manipulates it directly via
libgit2. Your working tree is never touched.

## Development

```sh
cargo test              # unit tests (todo serde, label parse, editor parse)
cargo build --release   # production binary at target/release/git-todo
git todo --generate-man # roff source to stdout
```

## Licence

MIT. See `LICENSE`.
