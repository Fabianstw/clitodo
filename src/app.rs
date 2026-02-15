use clap::{CommandFactory, Parser};
use std::cmp::Ordering;
use std::io::IsTerminal;

use crate::branch::{
    branch_matches, collect_branches, normalize_branch, normalize_state, print_branch_list,
};
use crate::cli::{Cli, Commands, ImportFormat};
use crate::display::{print_task_list, print_task_list_grouped, print_task_view};
use crate::edit::edit_interactive;
use crate::export::export_tasks;
use crate::model::{
    AppConfig, DEFAULT_BRANCH, IdScope, Priority, Repeat, SortKey, Task, default_branch,
};
use crate::sort::sort_tasks;
use crate::storage::{load_state, load_tasks, save_state, save_tasks, state_path, storage_path};
use crate::util::{advance_due, normalize_tag, normalize_tags, parse_bool_flag, parse_due};
use chrono::Local;
use clap_complete::generate;
use serde::Deserialize;
use uuid::Uuid;

pub fn run() {
    let cli = Cli::parse();
    let path = storage_path();
    let state_path = state_path();
    let mut state = load_state(&state_path);
    let mut tasks = load_tasks(&path);

    normalize_state(&mut state);
    let color = resolve_color(&cli, &state.config);

    match cli.command {
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            let name = cmd.get_name().to_string();
            generate(shell, &mut cmd, name, &mut std::io::stdout());
        }

        Commands::Branch { name, list } => {
            if list {
                print_branch_list(&state, &tasks);
                return;
            }

            if let Some(branch) = normalize_branch(name) {
                state.current_branch = branch;
                save_state(&state_path, &state);
                println!("Switched to {}", state.current_branch);
                return;
            }

            println!("Current branch: {}", state.current_branch);
        }

        Commands::BranchDelete { name, force } => {
            let Some(branch) = normalize_branch(Some(name)) else {
                eprintln!("Invalid branch name");
                std::process::exit(2);
            };

            if branch.eq_ignore_ascii_case(DEFAULT_BRANCH) {
                eprintln!("Cannot delete the default branch");
                std::process::exit(2);
            }

            let count = tasks
                .iter()
                .filter(|t| t.branch.eq_ignore_ascii_case(&branch))
                .count();
            if count > 0 && !force {
                eprintln!("Branch '{branch}' has {count} tasks. Use --force to delete.");
                std::process::exit(2);
            }

            let before = tasks.len();
            tasks.retain(|t| !t.branch.eq_ignore_ascii_case(&branch));
            let deleted = before - tasks.len();

            if state.current_branch.eq_ignore_ascii_case(&branch) {
                state.current_branch = default_branch();
                save_state(&state_path, &state);
            }

            save_tasks(&path, &tasks);
            println!("Deleted branch '{branch}' ({deleted} tasks removed)");
        }

        Commands::BranchMove { from, to } => {
            let Some(from) = normalize_branch(Some(from)) else {
                eprintln!("Invalid source branch");
                std::process::exit(2);
            };
            let Some(to) = normalize_branch(Some(to)) else {
                eprintln!("Invalid target branch");
                std::process::exit(2);
            };
            if from.eq_ignore_ascii_case(&to) {
                eprintln!("Source and target branch are the same");
                std::process::exit(2);
            }

            let mut moved = 0usize;
            let mut next_id = next_task_id(&tasks, state.config.id_scope, Some(&to));
            for task in tasks.iter_mut() {
                if task.branch.eq_ignore_ascii_case(&from) {
                    task.branch = to.clone();
                    if state.config.id_scope == IdScope::Branch {
                        task.id = next_id;
                        next_id += 1;
                    }
                    moved += 1;
                }
            }

            if moved == 0 {
                eprintln!("No tasks found in '{from}'");
                std::process::exit(1);
            }

            if state.current_branch.eq_ignore_ascii_case(&from) {
                state.current_branch = to.clone();
                save_state(&state_path, &state);
            }

            save_tasks(&path, &tasks);
            println!("Moved {moved} tasks from '{from}' to '{to}'");
        }

        Commands::BranchDuplicate { from, to } => {
            let Some(from) = normalize_branch(Some(from)) else {
                eprintln!("Invalid source branch");
                std::process::exit(2);
            };
            let Some(to) = normalize_branch(Some(to)) else {
                eprintln!("Invalid target branch");
                std::process::exit(2);
            };
            if from.eq_ignore_ascii_case(&to) {
                eprintln!("Source and target branch are the same");
                std::process::exit(2);
            }

            let mut next_id = next_task_id(&tasks, state.config.id_scope, Some(&to));
            let now = chrono::Local::now().to_rfc3339();
            let mut added = 0usize;
            let mut copies: Vec<Task> = Vec::new();

            for task in tasks.iter() {
                if task.branch.eq_ignore_ascii_case(&from) {
                    let mut copy = task.clone();
                    copy.id = next_id;
                    copy.branch = to.clone();
                    copy.created_at = now.clone();
                    if state.config.use_uuid {
                        copy.uid = Some(Uuid::new_v4().to_string());
                    }
                    next_id += 1;
                    added += 1;
                    copies.push(copy);
                }
            }

            if added == 0 {
                eprintln!("No tasks found in '{from}'");
                std::process::exit(1);
            }

            tasks.extend(copies);
            save_tasks(&path, &tasks);
            println!("Duplicated {added} tasks from '{from}' to '{to}'");
        }

        Commands::BranchMerge { from, to } => {
            let Some(from) = normalize_branch(Some(from)) else {
                eprintln!("Invalid source branch");
                std::process::exit(2);
            };
            let Some(to) = normalize_branch(Some(to)) else {
                eprintln!("Invalid target branch");
                std::process::exit(2);
            };
            if from.eq_ignore_ascii_case(&to) {
                eprintln!("Source and target branch are the same");
                std::process::exit(2);
            }

            let mut moved = 0usize;
            let mut next_id = next_task_id(&tasks, state.config.id_scope, Some(&to));
            for task in tasks.iter_mut() {
                if task.branch.eq_ignore_ascii_case(&from) {
                    task.branch = to.clone();
                    if state.config.id_scope == IdScope::Branch {
                        task.id = next_id;
                        next_id += 1;
                    }
                    moved += 1;
                }
            }

            if moved == 0 {
                eprintln!("No tasks found in '{from}'");
                std::process::exit(1);
            }

            if state.current_branch.eq_ignore_ascii_case(&from) {
                state.current_branch = to.clone();
                save_state(&state_path, &state);
            }

            save_tasks(&path, &tasks);
            println!("Merged '{from}' into '{to}' ({moved} tasks moved)");
        }

        Commands::Create {
            title,
            due,
            priority,
            content,
            repeat,
            branch,
            tags,
        } => {
            let due_date = due
                .as_deref()
                .map(parse_due)
                .transpose()
                .unwrap_or_else(|e| {
                    eprintln!("Invalid due date: {e}");
                    std::process::exit(2);
                });

            let created_at = chrono::Local::now().to_rfc3339();
            let branch = normalize_branch(branch).unwrap_or_else(|| state.current_branch.clone());
            let next_id = next_task_id(&tasks, state.config.id_scope, Some(&branch));
            let uid = if state.config.use_uuid {
                Some(Uuid::new_v4().to_string())
            } else {
                None
            };

            tasks.push(Task {
                id: next_id,
                uid,
                title,
                content,
                tags: normalize_tags(&tags),
                due: due_date,
                priority,
                repeat,
                branch,
                archived: false,
                done: false,
                created_at,
            });

            save_tasks(&path, &tasks);
            println!("Created task #{next_id}");
        }

        Commands::Edit {
            id,
            title,
            content,
            tags,
            remove_tags,
            due,
            priority,
            repeat,
            clear_content,
            clear_tags,
            clear_due,
            clear_priority,
            clear_repeat,
            branch,
        } => {
            let Some(task) = tasks.iter_mut().find(|t| t.id == id) else {
                eprintln!("No task with id {id}");
                std::process::exit(1);
            };

            let is_interactive = title.is_none()
                && content.is_none()
                && tags.is_empty()
                && remove_tags.is_empty()
                && !clear_tags
                && due.is_none()
                && priority.is_none()
                && repeat.is_none()
                && !clear_content
                && !clear_due
                && !clear_priority
                && !clear_repeat
                && branch.is_none();

            if is_interactive {
                edit_interactive(task);
            } else {
                if let Some(title) = title {
                    task.title = title;
                }
                if clear_content {
                    task.content = None;
                } else if let Some(content) = content {
                    task.content = Some(content);
                }
                if clear_tags {
                    task.tags.clear();
                } else {
                    apply_tag_changes(&mut task.tags, &tags, &remove_tags);
                }
                if clear_due {
                    task.due = None;
                } else if let Some(due) = due {
                    let due_date = parse_due(&due).unwrap_or_else(|e| {
                        eprintln!("Invalid due date: {e}");
                        std::process::exit(2);
                    });
                    task.due = Some(due_date);
                }
                if clear_priority {
                    task.priority = None;
                } else if let Some(priority) = priority {
                    task.priority = Some(priority);
                }
                if clear_repeat {
                    task.repeat = None;
                } else if let Some(repeat) = repeat {
                    task.repeat = Some(repeat);
                }
                if let Some(branch) = normalize_branch(branch) {
                    task.branch = branch;
                }
            }

            save_tasks(&path, &tasks);
            println!("Updated #{id}");
        }

        Commands::List {
            all,
            sort,
            desc,
            asc,
            branch,
            archived,
            tags,
        } => {
            let branch = normalize_branch(branch).unwrap_or_else(|| state.current_branch.clone());
            let sort = sort.unwrap_or(state.config.default_sort);
            let desc = resolve_desc(desc, asc, state.config.default_desc);
            let tags = normalize_tags(&tags);
            let mut view: Vec<&Task> = tasks
                .iter()
                .filter(|t| all || !t.done)
                .filter(|t| filter_archived(t, archived))
                .filter(|t| branch_matches(t, Some(&branch)))
                .filter(|t| tags_match(t, &tags))
                .collect();
            sort_tasks(&mut view, sort, desc);
            print_task_list(&view, color);
        }

        Commands::ListAll {
            all,
            sort,
            desc,
            asc,
            archived,
            tags,
        } => {
            let sort = sort.unwrap_or(state.config.default_sort);
            let desc = resolve_desc(desc, asc, state.config.default_desc);
            let tags = normalize_tags(&tags);
            let filtered: Vec<Task> = tasks
                .iter()
                .filter(|t| filter_archived(t, archived))
                .filter(|t| tags_match(t, &tags))
                .cloned()
                .collect();
            print_task_list_grouped(&filtered, &state, all, sort, desc, color);
        }

        Commands::ListRepeat {
            all,
            sort,
            desc,
            asc,
            branch,
            archived,
            tags,
        } => {
            let branch = normalize_branch(branch).unwrap_or_else(|| state.current_branch.clone());
            let sort = sort.unwrap_or(state.config.default_sort);
            let desc = resolve_desc(desc, asc, state.config.default_desc);
            let tags = normalize_tags(&tags);
            let mut view: Vec<&Task> = tasks
                .iter()
                .filter(|t| t.repeat.is_some())
                .filter(|t| all || !t.done)
                .filter(|t| filter_archived(t, archived))
                .filter(|t| branch_matches(t, Some(&branch)))
                .filter(|t| tags_match(t, &tags))
                .collect();
            sort_tasks(&mut view, sort, desc);
            print_task_list(&view, color);
        }

        Commands::ListDone {
            sort,
            desc,
            asc,
            branch,
            archived,
            tags,
        } => {
            let branch = normalize_branch(branch).unwrap_or_else(|| state.current_branch.clone());
            let sort = sort.unwrap_or(state.config.default_sort);
            let desc = resolve_desc(desc, asc, state.config.default_desc);
            let tags = normalize_tags(&tags);
            let mut view: Vec<&Task> = tasks
                .iter()
                .filter(|t| t.done)
                .filter(|t| filter_archived(t, archived))
                .filter(|t| branch_matches(t, Some(&branch)))
                .filter(|t| tags_match(t, &tags))
                .collect();
            sort_tasks(&mut view, sort, desc);
            print_task_list(&view, color);
        }

        Commands::Search {
            query,
            all,
            sort,
            desc,
            asc,
            branch,
            archived,
            tags,
        } => {
            let q = query.to_lowercase();
            let branch = normalize_branch(branch).unwrap_or_else(|| state.current_branch.clone());
            let sort = sort.unwrap_or(state.config.default_sort);
            let desc = resolve_desc(desc, asc, state.config.default_desc);
            let tags = normalize_tags(&tags);
            let mut view: Vec<&Task> = tasks
                .iter()
                .filter(|t| all || !t.done)
                .filter(|t| filter_archived(t, archived))
                .filter(|t| task_matches(t, &q))
                .filter(|t| branch_matches(t, Some(&branch)))
                .filter(|t| tags_match(t, &tags))
                .collect();

            sort_tasks(&mut view, sort, desc);
            print_task_list(&view, color);
        }

        Commands::Reminders { branch, tags } => {
            let branch = normalize_branch(branch).unwrap_or_else(|| state.current_branch.clone());
            let tags = normalize_tags(&tags);
            print_reminders(&tasks, &branch, &tags, state.config.reminder_days, color);
        }

        Commands::Stats => {
            print_stats(&tasks, &state.current_branch);
        }

        Commands::BulkDone { query, branch } => {
            let branch = normalize_branch(branch).unwrap_or_else(|| state.current_branch.clone());
            let count = bulk_set_done(
                &mut tasks,
                &query,
                &branch,
                true,
                state.config.id_scope,
                state.config.use_uuid,
            );
            if count == 0 {
                eprintln!("No matching tasks");
                std::process::exit(1);
            }
            save_tasks(&path, &tasks);
            println!("Marked {count} tasks as done");
        }

        Commands::BulkUndone { query, branch } => {
            let branch = normalize_branch(branch).unwrap_or_else(|| state.current_branch.clone());
            let count = bulk_set_done(
                &mut tasks,
                &query,
                &branch,
                false,
                state.config.id_scope,
                state.config.use_uuid,
            );
            if count == 0 {
                eprintln!("No matching tasks");
                std::process::exit(1);
            }
            save_tasks(&path, &tasks);
            println!("Marked {count} tasks as not done");
        }

        Commands::BulkEdit {
            query,
            due,
            priority,
            repeat,
            clear_due,
            clear_priority,
            clear_repeat,
            branch,
            tags,
            remove_tags,
            clear_tags,
        } => {
            let branch = normalize_branch(branch).unwrap_or_else(|| state.current_branch.clone());
            let opts = BulkEditOpts {
                due,
                priority,
                repeat,
                clear_due,
                clear_priority,
                clear_repeat,
                tags: &tags,
                remove_tags: &remove_tags,
                clear_tags,
            };
            let count = bulk_edit(&mut tasks, &query, &branch, &opts);
            if count == 0 {
                eprintln!("No matching tasks");
                std::process::exit(1);
            }
            save_tasks(&path, &tasks);
            println!("Updated {count} tasks");
        }

        Commands::BulkDelete { query, branch } => {
            let branch = normalize_branch(branch).unwrap_or_else(|| state.current_branch.clone());
            let count = bulk_delete(&mut tasks, &query, &branch);
            if count == 0 {
                eprintln!("No matching tasks");
                std::process::exit(1);
            }
            save_tasks(&path, &tasks);
            println!("Deleted {count} tasks");
        }

        Commands::BulkMove { query, to, branch } => {
            let Some(target) = normalize_branch(Some(to)) else {
                eprintln!("Invalid target branch");
                std::process::exit(2);
            };
            let branch = normalize_branch(branch).unwrap_or_else(|| state.current_branch.clone());
            if target.eq_ignore_ascii_case(&branch) {
                eprintln!("Source and target branch are the same");
                std::process::exit(2);
            }
            let count = bulk_move(&mut tasks, &query, &branch, &target, state.config.id_scope);
            if count == 0 {
                eprintln!("No matching tasks");
                std::process::exit(1);
            }
            save_tasks(&path, &tasks);
            println!("Moved {count} tasks to '{target}'");
        }

        Commands::Archive {
            id,
            all_branches,
            branch,
        } => {
            let count = if let Some(id) = id {
                archive_by_id(&mut tasks, id)
            } else if all_branches {
                archive_done_all(&mut tasks)
            } else {
                let branch =
                    normalize_branch(branch).unwrap_or_else(|| state.current_branch.clone());
                archive_done_branch(&mut tasks, &branch)
            };

            if count == 0 {
                eprintln!("No tasks archived");
                std::process::exit(1);
            }
            save_tasks(&path, &tasks);
            println!("Archived {count} tasks");
        }

        Commands::Unarchive { id } => {
            let Some(task) = tasks.iter_mut().find(|t| t.id == id) else {
                eprintln!("No task with id {id}");
                std::process::exit(1);
            };
            task.archived = false;
            save_tasks(&path, &tasks);
            println!("Unarchived #{id}");
        }

        Commands::Done { id } => {
            let Some(pos) = tasks.iter().position(|t| t.id == id) else {
                eprintln!("No task with id {id}");
                std::process::exit(1);
            };
            let branch = tasks[pos].branch.clone();
            let mut next_id = next_task_id(&tasks, state.config.id_scope, Some(&branch));
            let task = &mut tasks[pos];
            if let Some(next_task) =
                mark_done_with_repeat(task, &mut next_id, state.config.use_uuid)
            {
                tasks.push(next_task);
            }
            save_tasks(&path, &tasks);
            println!("Done #{id}");
        }

        Commands::Undone { id } => {
            let Some(task) = tasks.iter_mut().find(|t| t.id == id) else {
                eprintln!("No task with id {id}");
                std::process::exit(1);
            };
            task.done = false;
            save_tasks(&path, &tasks);
            println!("Undone #{id}");
        }

        Commands::Toggle { id } => {
            let Some(pos) = tasks.iter().position(|t| t.id == id) else {
                eprintln!("No task with id {id}");
                std::process::exit(1);
            };
            let branch = tasks[pos].branch.clone();
            let mut next_id = next_task_id(&tasks, state.config.id_scope, Some(&branch));
            let task = &mut tasks[pos];
            if task.done {
                task.done = false;
            } else if let Some(next_task) =
                mark_done_with_repeat(task, &mut next_id, state.config.use_uuid)
            {
                tasks.push(next_task);
            }
            save_tasks(&path, &tasks);
            println!("Toggled #{id}");
        }

        Commands::Delete { id } => {
            let len_before = tasks.len();
            tasks.retain(|t| t.id != id);
            if tasks.len() == len_before {
                eprintln!("No task with id {id}");
                std::process::exit(1);
            }
            save_tasks(&path, &tasks);
            println!("Deleted #{id}");
        }

        Commands::Clear => {
            let len_before = tasks.len();
            tasks.retain(|t| !t.done);
            let cleared = len_before - tasks.len();
            save_tasks(&path, &tasks);
            println!("Cleared {cleared} completed tasks");
        }

        Commands::View { id } => {
            let Some(task) = tasks.iter().find(|t| t.id == id) else {
                eprintln!("No task with id {id}");
                std::process::exit(1);
            };
            print_task_view(task, color);
        }

        Commands::Export {
            format,
            all,
            archived,
            branch,
            tags,
        } => {
            let branch = normalize_branch(branch).unwrap_or_else(|| state.current_branch.clone());
            let tags = normalize_tags(&tags);
            let view: Vec<&Task> = tasks
                .iter()
                .filter(|t| all || !t.done)
                .filter(|t| filter_archived(t, archived))
                .filter(|t| branch_matches(t, Some(&branch)))
                .filter(|t| tags_match(t, &tags))
                .collect();
            export_tasks(&view, format);
        }

        Commands::Import {
            format,
            file,
            branch,
        } => {
            let branch = normalize_branch(branch).unwrap_or_else(|| state.current_branch.clone());
            let result = import_tasks(&mut tasks, format, &file, &branch, &state.config);
            if result.imported == 0 {
                eprintln!("No tasks imported");
                std::process::exit(1);
            }
            save_tasks(&path, &tasks);
            println!("Imported {} tasks", result.imported);
            if result.skipped > 0 {
                eprintln!("Skipped {} rows", result.skipped);
                if result.skipped_title > 0 {
                    eprintln!("- Missing title: {}", result.skipped_title);
                }
                if result.skipped_parse > 0 {
                    eprintln!("- Parse errors: {}", result.skipped_parse);
                }
            }
        }

        Commands::Config {
            default_sort,
            default_desc,
            color: color_pref,
            reminder_days,
            id_scope,
            use_uuid,
        } => {
            let updated = update_config(
                &mut state.config,
                default_sort,
                default_desc,
                color_pref,
                reminder_days,
                id_scope,
                use_uuid,
            );
            if updated {
                save_state(&state_path, &state);
                println!("Updated config");
            } else {
                print_config(&state.config);
            }
        }
    }
}

fn task_matches(task: &Task, query: &str) -> bool {
    if task.title.to_lowercase().contains(query) {
        return true;
    }
    task.content
        .as_ref()
        .map(|c| c.to_lowercase().contains(query))
        .unwrap_or(false)
}

fn tags_match(task: &Task, tags: &[String]) -> bool {
    if tags.is_empty() {
        return true;
    }
    tags.iter()
        .any(|tag| task.tags.iter().any(|t| t.eq_ignore_ascii_case(tag)))
}

fn apply_tag_changes(tags: &mut Vec<String>, add: &[String], remove: &[String]) {
    let add = normalize_tags(add);
    let remove: Vec<String> = remove.iter().filter_map(|t| normalize_tag(t)).collect();

    if !add.is_empty() {
        for tag in add {
            if !tags.iter().any(|t| t.eq_ignore_ascii_case(&tag)) {
                tags.push(tag);
            }
        }
    }

    if !remove.is_empty() {
        tags.retain(|t| !remove.iter().any(|r| t.eq_ignore_ascii_case(r)));
    }

    tags.sort();
    tags.dedup();
}

fn next_task_id(tasks: &[Task], scope: IdScope, branch: Option<&str>) -> u64 {
    match scope {
        IdScope::Global => tasks.iter().map(|t| t.id).max().unwrap_or(0) + 1,
        IdScope::Branch => {
            let branch = branch.unwrap_or(DEFAULT_BRANCH);
            tasks
                .iter()
                .filter(|t| t.branch.eq_ignore_ascii_case(branch))
                .map(|t| t.id)
                .max()
                .unwrap_or(0)
                + 1
        }
    }
}

fn mark_done_with_repeat(task: &mut Task, next_id: &mut u64, use_uuid: bool) -> Option<Task> {
    if task.done {
        return None;
    }

    task.done = true;
    let repeat = task.repeat?;

    let base_due = task.due.unwrap_or_else(|| Local::now().date_naive());
    let next_due = advance_due(base_due, repeat)?;

    let mut copy = task.clone();
    copy.id = *next_id;
    copy.done = false;
    copy.due = Some(next_due);
    copy.created_at = Local::now().to_rfc3339();
    if use_uuid {
        copy.uid = Some(Uuid::new_v4().to_string());
    }
    *next_id += 1;
    Some(copy)
}

fn bulk_set_done(
    tasks: &mut Vec<Task>,
    query: &str,
    branch: &str,
    done: bool,
    id_scope: IdScope,
    use_uuid: bool,
) -> usize {
    let q = query.to_lowercase();
    let mut count = 0usize;
    let mut next_id = next_task_id(tasks, id_scope, Some(branch));
    let mut new_tasks: Vec<Task> = Vec::new();

    for task in tasks.iter_mut() {
        if !task.branch.eq_ignore_ascii_case(branch) {
            continue;
        }
        if task.archived {
            continue;
        }
        if !task_matches(task, &q) {
            continue;
        }

        if done {
            if task.done {
                continue;
            }
            if let Some(next_task) = mark_done_with_repeat(task, &mut next_id, use_uuid) {
                new_tasks.push(next_task);
            }
            count += 1;
        } else if task.done {
            task.done = false;
            count += 1;
        }
    }

    if !new_tasks.is_empty() {
        tasks.extend(new_tasks);
    }

    count
}

fn bulk_delete(tasks: &mut Vec<Task>, query: &str, branch: &str) -> usize {
    let q = query.to_lowercase();
    let before = tasks.len();
    tasks.retain(|task| {
        if !task.branch.eq_ignore_ascii_case(branch) {
            return true;
        }
        if task.archived {
            return true;
        }
        !task_matches(task, &q)
    });
    before - tasks.len()
}

struct BulkEditOpts<'a> {
    due: Option<String>,
    priority: Option<Priority>,
    repeat: Option<Repeat>,
    clear_due: bool,
    clear_priority: bool,
    clear_repeat: bool,
    tags: &'a [String],
    remove_tags: &'a [String],
    clear_tags: bool,
}

fn bulk_edit(tasks: &mut [Task], query: &str, branch: &str, opts: &BulkEditOpts) -> usize {
    if opts.due.is_none()
        && opts.priority.is_none()
        && opts.repeat.is_none()
        && opts.tags.is_empty()
        && opts.remove_tags.is_empty()
        && !opts.clear_due
        && !opts.clear_priority
        && !opts.clear_repeat
        && !opts.clear_tags
    {
        return 0;
    }

    let parsed_due = if let Some(value) = opts.due.as_deref() {
        match parse_due(value) {
            Ok(due) => Some(due),
            Err(e) => {
                eprintln!("Invalid due date: {e}");
                std::process::exit(2);
            }
        }
    } else {
        None
    };

    let q = query.to_lowercase();
    let mut count = 0usize;
    for task in tasks.iter_mut() {
        if !task.branch.eq_ignore_ascii_case(branch) {
            continue;
        }
        if task.archived {
            continue;
        }
        if !task_matches(task, &q) {
            continue;
        }

        let mut changed = false;
        if opts.clear_due {
            task.due = None;
            changed = true;
        } else if let Some(due) = parsed_due {
            task.due = Some(due);
            changed = true;
        }

        if opts.clear_priority {
            task.priority = None;
            changed = true;
        } else if let Some(priority) = opts.priority {
            task.priority = Some(priority);
            changed = true;
        }

        if opts.clear_repeat {
            task.repeat = None;
            changed = true;
        } else if let Some(repeat) = opts.repeat {
            task.repeat = Some(repeat);
            changed = true;
        }

        if opts.clear_tags {
            if !task.tags.is_empty() {
                task.tags.clear();
                changed = true;
            }
        } else if !opts.tags.is_empty() || !opts.remove_tags.is_empty() {
            let before = task.tags.clone();
            apply_tag_changes(&mut task.tags, opts.tags, opts.remove_tags);
            if task.tags != before {
                changed = true;
            }
        }

        if changed {
            count += 1;
        }
    }

    count
}

fn bulk_move(tasks: &mut [Task], query: &str, branch: &str, target: &str, scope: IdScope) -> usize {
    let q = query.to_lowercase();
    let mut count = 0usize;
    let mut next_id = next_task_id(tasks, scope, Some(target));
    for task in tasks.iter_mut() {
        if !task.branch.eq_ignore_ascii_case(branch) {
            continue;
        }
        if task.archived {
            continue;
        }
        if !task_matches(task, &q) {
            continue;
        }
        task.branch = target.to_string();
        if scope == IdScope::Branch {
            task.id = next_id;
            next_id += 1;
        }
        count += 1;
    }
    count
}

fn print_reminders(tasks: &[Task], branch: &str, tags: &[String], reminder_days: u32, color: bool) {
    let today = Local::now().date_naive();
    let mut overdue: Vec<&Task> = Vec::new();
    let mut today_list: Vec<&Task> = Vec::new();
    let mut upcoming: Vec<&Task> = Vec::new();

    for task in tasks.iter() {
        if task.done {
            continue;
        }
        if task.archived {
            continue;
        }
        if !task.branch.eq_ignore_ascii_case(branch) {
            continue;
        }
        if !tags_match(task, tags) {
            continue;
        }
        let Some(due) = task.due else {
            continue;
        };

        if due < today {
            overdue.push(task);
        } else if due == today {
            today_list.push(task);
        } else if reminder_days > 0 {
            let max = today.checked_add_days(chrono::Days::new(reminder_days as u64));
            if max.is_some_and(|max_due| due <= max_due) {
                upcoming.push(task);
            }
        }
    }

    if overdue.is_empty() && today_list.is_empty() && upcoming.is_empty() {
        println!("No reminders.");
        return;
    }

    if !overdue.is_empty() {
        println!("Overdue ({})", overdue.len());
        print_task_list(&overdue, color);
    }

    if !today_list.is_empty() {
        if !overdue.is_empty() {
            println!();
        }
        println!("Due today ({})", today_list.len());
        print_task_list(&today_list, color);
    }

    if !upcoming.is_empty() {
        if !overdue.is_empty() || !today_list.is_empty() {
            println!();
        }
        println!("Upcoming ({})", upcoming.len());
        print_task_list(&upcoming, color);
    }
}

fn print_stats(tasks: &[Task], current_branch: &str) {
    let today = Local::now().date_naive();
    let mut total = 0usize;
    let mut done = 0usize;
    let mut overdue = 0usize;
    let mut due_today = 0usize;
    let mut archived = 0usize;
    let mut repeating = 0usize;

    for task in tasks.iter() {
        total += 1;
        if task.archived {
            archived += 1;
        }
        if task.repeat.is_some() {
            repeating += 1;
        }
        if task.done {
            done += 1;
            continue;
        }
        if task.archived {
            continue;
        }
        if let Some(due) = task.due {
            match due.cmp(&today) {
                Ordering::Less => overdue += 1,
                Ordering::Equal => due_today += 1,
                Ordering::Greater => {}
            }
        }
    }

    println!("Total:    {total}");
    println!("Open:     {}", total - done);
    println!("Done:     {done}");
    println!("Overdue:  {overdue}");
    println!("Due today:{:>3}", due_today);
    println!("Archived: {archived}");
    println!("Repeating:{:>3}", repeating);

    let mut branches = collect_branches(tasks);
    branches.sort_by_key(|branch| branch.to_lowercase());
    if branches.is_empty() {
        return;
    }

    println!("\nBy branch:");
    for branch in branches {
        let mut b_total = 0usize;
        let mut b_done = 0usize;
        let mut b_archived = 0usize;
        for task in tasks.iter() {
            if task.branch.eq_ignore_ascii_case(&branch) {
                b_total += 1;
                if task.done {
                    b_done += 1;
                }
                if task.archived {
                    b_archived += 1;
                }
            }
        }
        let mark = if branch.eq_ignore_ascii_case(current_branch) {
            "*"
        } else {
            " "
        };
        println!(
            "{mark} {branch}: {}/{} done, {b_archived} archived",
            b_done, b_total
        );
    }
}

#[derive(Deserialize)]
struct ImportTaskJson {
    uid: Option<String>,
    title: Option<String>,
    content: Option<String>,
    tags: Option<Vec<String>>,
    due: Option<String>,
    priority: Option<crate::model::Priority>,
    repeat: Option<Repeat>,
    branch: Option<String>,
    done: Option<bool>,
    archived: Option<bool>,
    created_at: Option<String>,
}

#[derive(Deserialize)]
struct ImportTaskCsv {
    uid: Option<String>,
    title: String,
    content: Option<String>,
    tags: Option<String>,
    due: Option<String>,
    priority: Option<String>,
    repeat: Option<String>,
    branch: Option<String>,
    done: Option<String>,
    archived: Option<String>,
    created_at: Option<String>,
}

struct ImportResult {
    imported: usize,
    skipped: usize,
    skipped_title: usize,
    skipped_parse: usize,
}

fn import_tasks(
    tasks: &mut Vec<Task>,
    format: ImportFormat,
    file: &str,
    branch: &str,
    config: &AppConfig,
) -> ImportResult {
    match format {
        ImportFormat::Json => import_json(tasks, file, branch, config),
        ImportFormat::Csv => import_csv(tasks, file, branch, config),
    }
}

fn import_json(
    tasks: &mut Vec<Task>,
    file: &str,
    branch: &str,
    config: &AppConfig,
) -> ImportResult {
    let Ok(bytes) = std::fs::read(file) else {
        eprintln!("Failed to read file: {file}");
        std::process::exit(2);
    };
    let Ok(rows) = serde_json::from_slice::<Vec<ImportTaskJson>>(&bytes) else {
        eprintln!("Invalid JSON format");
        std::process::exit(2);
    };

    let mut next_id = next_task_id(tasks, config.id_scope, Some(branch));
    let mut branch_ids: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    let mut added = 0usize;
    let mut skipped = 0usize;
    let mut skipped_title = 0usize;

    for row in rows {
        let Some(title) = row.title else {
            skipped += 1;
            skipped_title += 1;
            continue;
        };
        let target_branch =
            normalize_branch(row.branch.clone()).unwrap_or_else(|| branch.to_string());
        let next_id_ref = if config.id_scope == IdScope::Branch {
            let entry = branch_ids
                .entry(target_branch.clone())
                .or_insert_with(|| next_task_id(tasks, config.id_scope, Some(&target_branch)));
            entry
        } else {
            &mut next_id
        };

        let tags = row.tags.unwrap_or_default();
        let task = build_task_from_parts(
            TaskParts {
                uid: row.uid,
                title,
                content: row.content,
                tags,
                due: row.due,
                priority: row.priority,
                repeat: row.repeat,
                branch: Some(target_branch.clone()),
                done: row.done,
                archived: row.archived,
                created_at: row.created_at,
            },
            branch,
            next_id_ref,
            config,
        );
        if let Some(task) = task {
            tasks.push(task);
            added += 1;
        } else {
            skipped += 1;
        }
    }

    ImportResult {
        imported: added,
        skipped,
        skipped_title,
        skipped_parse: 0,
    }
}

fn import_csv(tasks: &mut Vec<Task>, file: &str, branch: &str, config: &AppConfig) -> ImportResult {
    let mut rdr = match csv::Reader::from_path(file) {
        Ok(reader) => reader,
        Err(_) => {
            eprintln!("Failed to read file: {file}");
            std::process::exit(2);
        }
    };

    let mut next_id = next_task_id(tasks, config.id_scope, Some(branch));
    let mut branch_ids: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    let mut added = 0usize;
    let mut skipped = 0usize;
    let mut skipped_parse = 0usize;

    for result in rdr.deserialize::<ImportTaskCsv>() {
        let row = match result {
            Ok(row) => row,
            Err(_) => {
                skipped += 1;
                skipped_parse += 1;
                continue;
            }
        };

        let priority = row
            .priority
            .as_deref()
            .and_then(|p| p.parse::<crate::model::Priority>().ok());
        let repeat = row.repeat.as_deref().and_then(|r| r.parse::<Repeat>().ok());
        let done = row.done.as_deref().and_then(parse_bool_flag);
        let archived = row.archived.as_deref().and_then(parse_bool_flag);

        let target_branch =
            normalize_branch(row.branch.clone()).unwrap_or_else(|| branch.to_string());
        let next_id_ref = if config.id_scope == IdScope::Branch {
            let entry = branch_ids
                .entry(target_branch.clone())
                .or_insert_with(|| next_task_id(tasks, config.id_scope, Some(&target_branch)));
            entry
        } else {
            &mut next_id
        };

        let tags = row
            .tags
            .as_deref()
            .map(|raw| {
                raw.split(',')
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default();

        let task = build_task_from_parts(
            TaskParts {
                uid: row.uid,
                title: row.title,
                content: row.content,
                tags,
                due: row.due,
                priority,
                repeat,
                branch: Some(target_branch.clone()),
                done,
                archived,
                created_at: row.created_at,
            },
            branch,
            next_id_ref,
            config,
        );
        if let Some(task) = task {
            tasks.push(task);
            added += 1;
        } else {
            skipped += 1;
        }
    }

    ImportResult {
        imported: added,
        skipped,
        skipped_title: 0,
        skipped_parse,
    }
}

struct TaskParts {
    uid: Option<String>,
    title: String,
    content: Option<String>,
    tags: Vec<String>,
    due: Option<String>,
    priority: Option<Priority>,
    repeat: Option<Repeat>,
    branch: Option<String>,
    done: Option<bool>,
    archived: Option<bool>,
    created_at: Option<String>,
}

fn build_task_from_parts(
    parts: TaskParts,
    default_branch: &str,
    next_id: &mut u64,
    config: &AppConfig,
) -> Option<Task> {
    if parts.title.trim().is_empty() {
        return None;
    }

    let due = match parts.due.as_deref() {
        Some(value) => match parse_due(value) {
            Ok(due) => Some(due),
            Err(_) => None,
        },
        None => None,
    };

    let branch = normalize_branch(parts.branch).unwrap_or_else(|| default_branch.to_string());
    let created_at = parts
        .created_at
        .unwrap_or_else(|| Local::now().to_rfc3339());
    let done = parts.done.unwrap_or(false);
    let archived = parts.archived.unwrap_or(false);
    let uid = parts.uid.or_else(|| {
        if config.use_uuid {
            Some(Uuid::new_v4().to_string())
        } else {
            None
        }
    });

    let tags = normalize_tags(&parts.tags);

    let task = Task {
        id: *next_id,
        uid,
        title: parts.title,
        content: parts.content,
        tags,
        due,
        priority: parts.priority,
        repeat: parts.repeat,
        branch,
        archived,
        done,
        created_at,
    };

    *next_id += 1;
    Some(task)
}

fn resolve_desc(desc: bool, asc: bool, default_desc: bool) -> bool {
    if asc {
        return false;
    }
    if desc {
        return true;
    }
    default_desc
}

fn resolve_color(cli: &Cli, config: &AppConfig) -> bool {
    if cli.no_color {
        return false;
    }
    config.color && std::io::stdout().is_terminal()
}

fn filter_archived(task: &Task, include_archived: bool) -> bool {
    if include_archived {
        true
    } else {
        !task.archived
    }
}

fn archive_by_id(tasks: &mut [Task], id: u64) -> usize {
    let Some(task) = tasks.iter_mut().find(|t| t.id == id) else {
        return 0;
    };
    task.archived = true;
    1
}

fn archive_done_branch(tasks: &mut [Task], branch: &str) -> usize {
    let mut count = 0usize;
    for task in tasks.iter_mut() {
        if task.branch.eq_ignore_ascii_case(branch) && task.done && !task.archived {
            task.archived = true;
            count += 1;
        }
    }
    count
}

fn archive_done_all(tasks: &mut [Task]) -> usize {
    let mut count = 0usize;
    for task in tasks.iter_mut() {
        if task.done && !task.archived {
            task.archived = true;
            count += 1;
        }
    }
    count
}

fn update_config(
    config: &mut AppConfig,
    default_sort: Option<SortKey>,
    default_desc: Option<bool>,
    color: Option<bool>,
    reminder_days: Option<u32>,
    id_scope: Option<IdScope>,
    use_uuid: Option<bool>,
) -> bool {
    let mut changed = false;
    if let Some(default_sort) = default_sort {
        config.default_sort = default_sort;
        changed = true;
    }
    if let Some(default_desc) = default_desc {
        config.default_desc = default_desc;
        changed = true;
    }
    if let Some(color) = color {
        config.color = color;
        changed = true;
    }
    if let Some(reminder_days) = reminder_days {
        config.reminder_days = reminder_days;
        changed = true;
    }
    if let Some(id_scope) = id_scope {
        config.id_scope = id_scope;
        changed = true;
    }
    if let Some(use_uuid) = use_uuid {
        config.use_uuid = use_uuid;
        changed = true;
    }
    changed
}

fn print_config(config: &AppConfig) {
    println!("default_sort: {:?}", config.default_sort);
    println!("default_desc: {}", config.default_desc);
    println!("color:        {}", config.color);
    println!("reminder_days:{:>3}", config.reminder_days);
    println!("id_scope:     {:?}", config.id_scope);
    println!("use_uuid:     {}", config.use_uuid);
}
