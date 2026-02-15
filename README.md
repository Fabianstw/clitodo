# todo

A fast, local-first CLI todo app built in Rust. Tasks support optional due date, priority, and content, with sorting, searching, and exports.

## Features

- Create, edit, delete, and view tasks
- Optional fields: due date, priority, content
- Tags for filtering
- Interactive edit mode
- List/search with sorting and filters
- Toggle done state, list completed tasks
- Branches with switch, merge, and move
- Recurring tasks and reminders
- Bulk updates and stats
- Import from JSON or CSV
- Archive done tasks
- Configurable defaults and shell completions
- Export to JSON, Markdown, or text

## Install

```bash
cargo build --release
```
or adding to PATH:

```bash
cargo install --path .
```

Run with:

```bash
cargo run -- <command>
```

## Quick start

- Import the sample data: `todo import --format json --file start.json`
- Follow the walkthrough in [example.md](example.md)
- Sample data file: [start.json](start.json)

## CLI guide

- LaTeX source: [docs/cli-guide.tex](docs/cli-guide.tex)
- PDF (generated): [docs/cli-guide.pdf](docs/cli-guide.pdf)

## Usage

```bash
todo <command> [options]
```

Global options:

```bash
--no-color   Disable colored output
```

### Create

```bash
todo create "Go shopping" -c "Milk, eggs" -d tomorrow -p high
todo c "Call mom"
todo create "Water plants" --repeat weekly
todo create "Pack" --tag travel --tag urgent
```

### List

```bash
todo list
todo l -a
todo list --sort priority --desc
todo list --archived
todo list --tag urgent
todo list-all
todo list-repeat
```

### Search

```bash
todo search "shopping"
todo s "milk" -a --sort due
todo search "meeting" --tag work
```

### View

```bash
todo view 3
todo v 3
```

### Edit

Interactive edit (no flags):

```bash
todo edit 3
```

Non-interactive edit:

```bash
todo edit 3 --title "Go shopping" --content "Milk, eggs"
todo edit 3 --clear-content --clear-due
todo edit 3 --repeat daily
```

### Done / Undone / Toggle

```bash
todo done 3
todo undone 3
todo toggle 3
```

### Bulk operations

```bash
todo bulk-done "shopping"
todo bulk-undone "shopping"
todo bulk-delete "shopping"
todo bulk-move "shopping" work
todo bulk-edit "shopping" --priority high --clear-due
todo bulk-edit "shopping" --tag urgent --remove-tag later
```

### List completed

```bash
todo list-done
```

### Export

```bash
todo export json
todo export markdown
todo export text
todo export json --tag urgent
```

### Archive

```bash
todo archive
todo archive --all-branches
todo archive 42
todo unarchive 42
```

### Import

```bash
todo import --format json --file tasks.json
todo import --format csv --file tasks.csv
```

### Config

```bash
todo config
todo config --default-sort priority --default-desc true
todo config --color false
todo config --reminder-days 3
todo config --id-scope branch
todo config --use-uuid true
```

### Completions

```bash
todo completions bash
todo completions zsh
todo completions fish
```

### Reminders and stats

```bash
todo reminders
todo stats
```

### Branches

```bash
todo branch work
todo branch
todo branch -l
todo branch-delete work --force
todo branch-move work personal
todo branch-dup work personal
todo branch-merge work personal
```

## Due date formats

- today
- tomorrow
- YYYY-MM-DD
- DDMMYYYY (e.g. 18022026)

## Short aliases

- branch: b, switch
- create: c, add
- list: l, ls
- list-all: la, list-all
- list-repeat: lr, list-repeat
- search: s, find
- reminders: r, remind, due
- stats: st, stats
- bulk-done: bd, bulk-done
- bulk-undone: bu, bulk-undone
- bulk-delete: bx, bulk-delete
- bulk-move: bm, bulk-move
- bulk-edit: be, bulk-edit
- edit: e
- done: d
- undone: u
- toggle: t
- delete: x, rm, del
- clear: clr
- view: v
- export: ex, out
- import: imp, in
- archive: arc, archive-done
- unarchive: unarc, unarchive
- completions: comp, completion, completions
- config: cfg, config

## Branch model

- The current branch is the default filter for list/search/export.
- Use `-b` to override the current branch for a command.
- New tasks default to the current branch.

## Recurring tasks

- Use `--repeat daily|weekly|monthly` on create or edit.
- When a repeating task is marked done, the next occurrence is created automatically.

## Tags

- Add tags with `--tag` (repeatable).
- Filter list/search/export with `--tag`.

## Archived tasks

- `archive` moves completed tasks to the archive instead of deleting them.
- Use `--archived` to include archived tasks in list/search/export.

## IDs and UUIDs

- `id-scope` can be `global` (default) or `branch`.
- `use-uuid` adds a stable UUID to each task for safer merges/imports.

## Data storage

Tasks are stored as JSON under your user data directory:

- macOS: ~/Library/Application Support/todo/tasks.json
- macOS: ~/Library/Application Support/todo/state.json

## Developer notes

- See [CONTRIBUTING.md](CONTRIBUTING.md) for formatting and testing commands.
