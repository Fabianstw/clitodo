# clitodo

A fast, local-first CLI todo app.

- Stores tasks locally
- Supports due dates, priorities, tags, branches, recurring tasks
- Includes a nicer terminal list view + optional daily greeting

Full documentation will live on a separate website (link coming soon).

## Install

macOS / Linux:

```bash
curl -fsSL https://raw.githubusercontent.com/Fabianstw/clitodo/main/install.sh | sh
```

Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/Fabianstw/clitodo/main/install.ps1 | iex
```

## Examples

Create tasks:

```bash
todo create "Go shopping" -d tomorrow -p high
todo create "Pack" --tag travel --tag urgent
```

List and search:

```bash
todo list
todo list --group-by due-day
todo search "shopping"
```

Split tasks by whether a due date is set:

```bash
todo split-due
```

Save a command under your own name:

```bash
todo saved save today -- list --group-by due-day
todo today
```

## Quick start data
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
- settings: set, settings, profile, me

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
