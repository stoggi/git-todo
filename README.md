# git-todo

Track todos as commits on a `todo` branch — a tiny, opinionated git-native
todo tracker written in Rust.

Inspired by [git-bug](https://github.com/MichaelMure/git-bug), but deliberately
stripped down. git-bug's design is powerful (event-sourced DAG, Lamport
clocks, distributed identity) — overkill if all you want is a todo list that
lives next to your code. git-todo keeps the parts that matter and drops the
rest:

- **Todos live on a visible `todo` branch**, not hidden refs. `git log todo`,
  `git show todo:todos/<id>.toml`, and GitHub's branch view all just work.
- **Sync is plain `git push origin todo` / `git pull origin todo`** — no
  custom refspecs, no separate sync command.
- **Identity comes from `git config user.name` / `user.email`** — no separate
  identity layer to manage.
- **Works as a git subcommand**: `git todo new`, `git todo done`, etc. — the
  binary is named `git-todo` and git's built-in subcommand discovery does the
  rest.

## Install

### From source

```sh
cargo install --path .
```

Installs `git-todo` to `~/.cargo/bin/`. The man page isn't installed by cargo;
generate and install it separately:

```sh
git todo --generate-man | sudo tee /usr/local/share/man/man1/git-todo.1 > /dev/null
sudo mandb -q
```

After that, `man git-todo` and `git todo --help` both work.

### Arch Linux (AUR)

A `git-todo-git` PKGBUILD is included under `packaging/arch/`. Once published
to the AUR, install with your favourite helper:

```sh
yay -S git-todo-git
```

### Homebrew

```sh
brew install --HEAD https://raw.githubusercontent.com/stoggi/git-todo/main/packaging/homebrew/git-todo.rb
```

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
created_by = "Jeremy Stott <jeremy@stott.co.nz>"
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
comment: 74885aea by Jeremy Stott <jeremy@stott.co.nz>
label: 74885aea +chore +shop
new: Buy milk (74885aea)
```

The `todo` branch is never checked out — git-todo manipulates it directly via
libgit2. Your working tree is never touched.

## Development

```sh
cargo test              # unit tests (todo serde, label parse, editor parse)
cargo build --release   # production binary at target/release/git-todo
git todo --generate-man # roff source to stdout
```

## Licence

MIT. See `LICENSE`.
