# Example walkthrough

This guide shows a quick start using the provided start.json file.

## 1) Import sample data

```bash
todo import --format json --file start.json
```

## 2) List tasks

```bash
todo list
```

## 3) Switch branches

```bash
todo branch work
todo list
```

## 4) Use tags

```bash
todo list --tag urgent
todo search "meeting" --tag work
```

## 5) Mark a task done

```bash
todo done 1
```

## 6) Reminders

```bash
todo reminders
```

## 7) Recurring tasks

```bash
todo list-repeat
```

## 8) Export

```bash
todo export json
```

## Notes

- The current branch is the default filter for list/search/export.
- Use `todo branch -l` to see known branches.
- Use `todo config` to see defaults.
