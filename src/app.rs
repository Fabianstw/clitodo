use clap::{CommandFactory, Parser};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::io::{IsTerminal, Write};

use crate::branch::{
    branch_matches, collect_branches, normalize_branch, normalize_state, print_branch_list,
};
use crate::cli::{Cli, Commands, GroupBy, ImportFormat, SavedCommands};
use crate::display::{
    print_task_list, print_task_list_due_split, print_task_list_grouped, print_task_view,
};
use crate::edit::edit_interactive;
use crate::export::export_tasks;
use crate::model::{
    AppConfig, DEFAULT_BRANCH, IdScope, Priority, Repeat, SortKey, Task, default_branch,
};
use crate::sort::sort_tasks;
use crate::storage::{load_state, load_tasks, save_state, save_tasks, state_path, storage_path};
use crate::util::{advance_due, normalize_tag, normalize_tags, parse_bool_flag, parse_due};
use chrono::{Datelike, Duration, Local, Timelike};
use clap_complete::generate;
use owo_colors::OwoColorize;
use serde::Deserialize;
use uuid::Uuid;

pub fn run() {
    let raw_args: Vec<String> = std::env::args().collect();

    let path = storage_path();
    let state_path = state_path();
    let mut state = load_state(&state_path);
    normalize_state(&mut state);

    let expanded_args = expand_saved_command_args(&raw_args, &state);
    let cli = Cli::parse_from(expanded_args);

    if let Commands::Saved { command } = &cli.command {
        handle_saved_commands(command, &mut state, &state_path);
        return;
    }

    let mut tasks = load_tasks(&path);
    let color = resolve_color(&cli, &state.config);
    maybe_print_daily_greeting(&mut state, &tasks, color, &state_path, &cli.command);

    match cli.command {
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            let name = cmd.get_name().to_string();
            generate(shell, &mut cmd, name, &mut std::io::stdout());
        }

        Commands::Saved { .. } => {
            // Handled before loading tasks.
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
            group_by,
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
            let group_by_day = matches!(group_by, Some(GroupBy::DueDay));
            print_task_list(&view, &state, color, group_by_day);
        }

        Commands::SplitDue {
            all,
            archived,
            branch,
            tags,
            sort,
            desc,
            asc,
        } => {
            let branch = normalize_branch(branch).unwrap_or_else(|| state.current_branch.clone());
            let sort = sort.unwrap_or(state.config.default_sort);
            let desc = resolve_desc(desc, asc, state.config.default_desc);
            let tags = normalize_tags(&tags);

            let mut due_view: Vec<&Task> = tasks
                .iter()
                .filter(|t| all || !t.done)
                .filter(|t| t.due.is_some())
                .filter(|t| filter_archived(t, archived))
                .filter(|t| branch_matches(t, Some(&branch)))
                .filter(|t| tags_match(t, &tags))
                .collect();

            let mut no_due_view: Vec<&Task> = tasks
                .iter()
                .filter(|t| all || !t.done)
                .filter(|t| t.due.is_none())
                .filter(|t| filter_archived(t, archived))
                .filter(|t| branch_matches(t, Some(&branch)))
                .filter(|t| tags_match(t, &tags))
                .collect();

            sort_tasks(&mut due_view, sort, desc);
            sort_tasks(&mut no_due_view, sort, desc);

            print_task_list_due_split(&due_view, &no_due_view, &state, color);
        }

        Commands::ListAll {
            all,
            sort,
            desc,
            asc,
            archived,
            tags,
            group_by,
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
            let group_by_day = matches!(group_by, Some(GroupBy::DueDay));
            print_task_list_grouped(&filtered, &state, all, sort, desc, color, group_by_day);
        }

        Commands::ListRepeat {
            all,
            sort,
            desc,
            asc,
            branch,
            archived,
            tags,
            group_by,
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
            let group_by_day = matches!(group_by, Some(GroupBy::DueDay));
            print_task_list(&view, &state, color, group_by_day);
        }

        Commands::ListDone {
            sort,
            desc,
            asc,
            branch,
            archived,
            tags,
            group_by,
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
            let group_by_day = matches!(group_by, Some(GroupBy::DueDay));
            print_task_list(&view, &state, color, group_by_day);
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
            group_by,
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
            let group_by_day = matches!(group_by, Some(GroupBy::DueDay));
            print_task_list(&view, &state, color, group_by_day);
        }

        Commands::Reminders { branch, tags } => {
            let branch = normalize_branch(branch).unwrap_or_else(|| state.current_branch.clone());
            let tags = normalize_tags(&tags);
            print_reminders(
                &tasks,
                &state,
                &branch,
                &tags,
                state.config.reminder_days,
                color,
            );
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

        Commands::Settings {
            name,
            clear_name,
            message,
            clear_message,
            daily_greeting,
            day_start_hour,
            greeting_style,
            greeting_summary,
            summary_scope,
            encouragement,
            pronouns,
            clear_pronouns,
            list_view,
            columns,
            columns_default,
            auto_pager,
            reset_greeting,
        } => {
            let mut changed = false;

            if clear_name {
                state.profile.name = None;
                changed = true;
            }
            if let Some(name) = name {
                let trimmed = name.trim();
                state.profile.name = if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                };
                changed = true;
            }

            if clear_message {
                state.profile.daily_message = None;
                changed = true;
            }
            if let Some(message) = message {
                let trimmed = message.trim();
                state.profile.daily_message = if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                };
                changed = true;
            }

            if let Some(enabled) = daily_greeting {
                state.profile.daily_greeting = enabled;
                changed = true;
            }

            if let Some(hour) = day_start_hour {
                if hour <= 23 {
                    state.profile.day_start_hour = hour;
                    changed = true;
                } else {
                    eprintln!("day-start-hour must be between 0 and 23");
                    std::process::exit(2);
                }
            }

            if let Some(style) = greeting_style {
                state.profile.greeting_style = style;
                changed = true;
            }

            if let Some(enabled) = greeting_summary {
                state.profile.greeting_summary = enabled;
                changed = true;
            }

            if let Some(scope) = summary_scope {
                state.profile.summary_scope = scope;
                changed = true;
            }

            if let Some(mode) = encouragement {
                state.profile.encouragement_mode = mode;
                changed = true;
            }

            if clear_pronouns {
                state.profile.pronouns = None;
                changed = true;
            }
            if let Some(pronouns) = pronouns {
                let trimmed = pronouns.trim();
                state.profile.pronouns = if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                };
                changed = true;
            }

            if let Some(view) = list_view {
                state.profile.list_view = view;
                changed = true;
            }

            if columns_default {
                state.profile.list_columns = crate::model::default_list_columns();
                changed = true;
            } else if !columns.is_empty() {
                let mut unique: Vec<crate::model::ListColumn> = Vec::new();
                for col in columns {
                    if !unique.contains(&col) {
                        unique.push(col);
                    }
                }
                state.profile.list_columns = unique;
                changed = true;
            }

            if let Some(enabled) = auto_pager {
                state.profile.auto_pager = enabled;
                changed = true;
            }
            if reset_greeting {
                state.profile.last_greeted = None;
                changed = true;
            }

            if changed {
                save_state(&state_path, &state);
                println!("Updated settings");
            }
            print_settings(&state, color);
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

fn reserved_top_level_command_names() -> HashSet<String> {
    let mut reserved: HashSet<String> = HashSet::new();
    let cmd = Cli::command();
    for sub in cmd.get_subcommands() {
        reserved.insert(sub.get_name().to_lowercase());
        for alias in sub.get_all_aliases() {
            reserved.insert(alias.to_lowercase());
        }
    }

    // Clap built-ins / common expectations.
    reserved.insert("help".to_string());
    reserved.insert("version".to_string());

    reserved
}

fn expand_saved_command_args(raw_args: &[String], state: &crate::model::AppState) -> Vec<String> {
    if raw_args.len() < 2 {
        return raw_args.to_vec();
    }

    let first = raw_args[1].as_str();
    if first.starts_with('-') {
        return raw_args.to_vec();
    }

    let reserved = reserved_top_level_command_names();
    if reserved.contains(&first.to_lowercase()) {
        return raw_args.to_vec();
    }

    let saved_key = state
        .profile
        .saved_commands
        .keys()
        .find(|k| k.eq_ignore_ascii_case(first))
        .cloned();

    let Some(saved_key) = saved_key else {
        return raw_args.to_vec();
    };
    let Some(saved_args) = state.profile.saved_commands.get(&saved_key) else {
        return raw_args.to_vec();
    };
    if saved_args.is_empty() {
        return raw_args.to_vec();
    }

    let mut expanded: Vec<String> = Vec::with_capacity(1 + saved_args.len() + raw_args.len());
    expanded.push(raw_args[0].clone());
    expanded.extend(saved_args.iter().cloned());
    expanded.extend(raw_args.iter().skip(2).cloned());
    expanded
}

fn handle_saved_commands(
    command: &SavedCommands,
    state: &mut crate::model::AppState,
    state_path: &std::path::PathBuf,
) {
    match command {
        SavedCommands::List => {
            if state.profile.saved_commands.is_empty() {
                println!("No saved commands.");
                return;
            }

            for (name, args) in state.profile.saved_commands.iter() {
                println!("{name}: {}", args.join(" "));
            }
        }

        SavedCommands::Show { name } => {
            let key = state
                .profile
                .saved_commands
                .keys()
                .find(|k| k.eq_ignore_ascii_case(name))
                .cloned();
            let Some(key) = key else {
                eprintln!("No saved command named '{name}'");
                std::process::exit(1);
            };
            let args = state
                .profile
                .saved_commands
                .get(&key)
                .cloned()
                .unwrap_or_default();
            println!("{key}: todo {}", args.join(" "));
        }

        SavedCommands::Remove { name } => {
            let key = state
                .profile
                .saved_commands
                .keys()
                .find(|k| k.eq_ignore_ascii_case(name))
                .cloned();
            let Some(key) = key else {
                eprintln!("No saved command named '{name}'");
                std::process::exit(1);
            };
            state.profile.saved_commands.remove(&key);
            save_state(state_path, state);
            println!("Removed saved command '{key}'");
        }

        SavedCommands::Save { name, args } => {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                eprintln!("Saved command name cannot be empty");
                std::process::exit(2);
            }
            if trimmed.starts_with('-') {
                eprintln!("Saved command name cannot start with '-'");
                std::process::exit(2);
            }

            let reserved = reserved_top_level_command_names();
            if reserved.contains(&trimmed.to_lowercase()) {
                eprintln!("'{trimmed}' is a built-in command name/alias and cannot be overwritten");
                std::process::exit(2);
            }

            if args.is_empty() {
                eprintln!(
                    "No command provided. Example: todo saved save today -- list --group-by due-day"
                );
                std::process::exit(2);
            }

            let existing_key = state
                .profile
                .saved_commands
                .keys()
                .find(|k| k.eq_ignore_ascii_case(trimmed))
                .cloned();
            if let Some(existing_key) = existing_key {
                if !confirm_overwrite(&format!("Overwrite saved command '{existing_key}'?")) {
                    println!("Not overwritten.");
                    return;
                }
                state.profile.saved_commands.remove(&existing_key);
            }

            state
                .profile
                .saved_commands
                .insert(trimmed.to_string(), args.clone());
            save_state(state_path, state);
            println!("Saved command '{trimmed}'");
        }
    }
}

fn confirm_overwrite(prompt: &str) -> bool {
    if !std::io::stdin().is_terminal() {
        return false;
    }

    let mut stdout = std::io::stdout();
    let _ = write!(stdout, "{prompt} [y/N]: ");
    let _ = stdout.flush();

    let mut input = String::new();
    if std::io::stdin().read_line(&mut input).is_err() {
        return false;
    }

    matches!(input.trim(), "y" | "Y" | "yes" | "YES" | "Yes")
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

fn print_reminders(
    tasks: &[Task],
    state: &crate::model::AppState,
    branch: &str,
    tags: &[String],
    reminder_days: u32,
    color: bool,
) {
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
        print_task_list(&overdue, state, color, false);
    }

    if !today_list.is_empty() {
        if !overdue.is_empty() {
            println!();
        }
        println!("Due today ({})", today_list.len());
        print_task_list(&today_list, state, color, false);
    }

    if !upcoming.is_empty() {
        if !overdue.is_empty() || !today_list.is_empty() {
            println!();
        }
        println!("Upcoming ({})", upcoming.len());
        print_task_list(&upcoming, state, color, false);
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

fn maybe_print_daily_greeting(
    state: &mut crate::model::AppState,
    tasks: &[Task],
    color: bool,
    state_path: &std::path::PathBuf,
    command: &Commands,
) {
    if matches!(command, Commands::Completions { .. }) {
        return;
    }
    if !state.profile.daily_greeting {
        return;
    }
    if !std::io::stdout().is_terminal() {
        return;
    }

    let now = Local::now();
    let day_key = (now - Duration::hours(state.profile.day_start_hour as i64)).date_naive();
    if state.profile.last_greeted == Some(day_key) {
        return;
    }

    let hour = now.hour();
    let salutation = if hour < 12 {
        "Good morning"
    } else if hour < 18 {
        "Good afternoon"
    } else {
        "Good evening"
    };

    let name = state
        .profile
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("friend");

    let greeting_text = format!("{salutation}, {name}!");
    let message_text = greeting_message(state, day_key);

    let summary_text = if state.profile.greeting_summary {
        let (open, overdue, due_today) = match state.profile.summary_scope {
            crate::model::SummaryScope::Current => {
                task_summary_current_branch(tasks, &state.current_branch)
            }
            crate::model::SummaryScope::All => task_summary_all(tasks),
        };

        if open > 0 {
            Some(format!(
                "Today: {open} open task{} · {overdue} overdue · {due_today} due today",
                if open == 1 { "" } else { "s" },
            ))
        } else {
            None
        }
    } else {
        None
    };

    match state.profile.greeting_style {
        crate::model::GreetingStyle::Banner => {
            let term_width = terminal_width().unwrap_or(80).clamp(40, 200);
            let banner_width =
                std::cmp::min(std::cmp::max(44, std::cmp::min(78, term_width)), term_width);
            let inner_width = banner_width.saturating_sub(2);

            let top = format!("┏{}┓", "━".repeat(inner_width));
            let bottom = format!("┗{}┛", "━".repeat(inner_width));
            let greet_line = format!("┃{}┃", center_in_width(&greeting_text, inner_width));
            let msg_line = format!("┃{}┃", center_in_width(&message_text, inner_width));
            let summary_line = summary_text
                .as_ref()
                .map(|s| format!("┃{}┃", center_in_width(s, inner_width)));

            println!();
            print_banner_line(&top, term_width, color, BannerStyle::Border);
            print_banner_line(&greet_line, term_width, color, BannerStyle::Greeting);
            if !message_text.is_empty() {
                print_banner_line(&msg_line, term_width, color, BannerStyle::Message);
            }
            if let Some(line) = summary_line {
                print_banner_line(&line, term_width, color, BannerStyle::Summary);
            }
            print_banner_line(&bottom, term_width, color, BannerStyle::Border);
        }

        crate::model::GreetingStyle::Compact => {
            let term_width = terminal_width().unwrap_or(80).clamp(40, 200);
            let line = if message_text.is_empty() {
                greeting_text.clone()
            } else {
                format!("{greeting_text} — {message_text}")
            };

            if color {
                println!("{}", center_line(&line, term_width).bright_yellow().bold());
            } else {
                println!("{}", center_line(&line, term_width));
            }

            if let Some(summary) = summary_text {
                if color {
                    println!(
                        "{}",
                        center_line(&summary, term_width).bright_cyan().dimmed()
                    );
                } else {
                    println!("{}", center_line(&summary, term_width));
                }
            }
        }
    }

    if state
        .profile
        .name
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        let tip = "Tip: set your name with: todo settings --name \"Your Name\"";
        let term_width = terminal_width().unwrap_or(80).clamp(40, 200);
        let tip_line = center_line(tip, term_width);
        if color {
            println!("{}", tip_line.dimmed());
        } else {
            println!("{tip_line}");
        }
    }

    println!();

    state.profile.last_greeted = Some(day_key);
    save_state(state_path, state);
}

fn greeting_message(state: &crate::model::AppState, day_key: chrono::NaiveDate) -> String {
    if let Some(msg) = state.profile.daily_message.as_deref().map(str::trim) {
        if !msg.is_empty() {
            return msg.to_string();
        }
    }

    match state.profile.encouragement_mode {
        crate::model::EncouragementMode::Off => String::new(),
        crate::model::EncouragementMode::CustomOnly => String::new(),
        crate::model::EncouragementMode::BuiltIn => built_in_encouragement(day_key).to_string(),
    }
}

fn built_in_encouragement(day_key: chrono::NaiveDate) -> &'static str {
    const LINES: &[&str] = &[
        "You’ve got this.",
        "One small step is still progress.",
        "Be kind to yourself today.",
        "Start with the easiest win.",
        "Future-you will thank you.",
        "Make it simple. Make it real.",
        "Momentum beats perfection.",
    ];

    let idx = (day_key.num_days_from_ce() as usize) % LINES.len();
    LINES[idx]
}

#[derive(Copy, Clone)]
enum BannerStyle {
    Border,
    Greeting,
    Message,
    Summary,
}

fn print_banner_line(line: &str, term_width: usize, color: bool, style: BannerStyle) {
    let centered = center_line(line, term_width);
    if !color {
        println!("{centered}");
        return;
    }

    match style {
        BannerStyle::Border => println!("{}", centered.bright_blue().bold()),
        BannerStyle::Greeting => println!("{}", centered.bright_yellow().bold()),
        BannerStyle::Message => println!("{}", centered.bright_magenta().bold()),
        BannerStyle::Summary => println!("{}", centered.bright_cyan().dimmed()),
    }
}

fn terminal_width() -> Option<usize> {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|w| *w > 0)
}

fn center_line(text: &str, width: usize) -> String {
    let len = text.chars().count();
    if width <= len {
        return text.to_string();
    }
    let pad = (width - len) / 2;
    format!("{}{}", " ".repeat(pad), text)
}

fn center_in_width(text: &str, width: usize) -> String {
    let len = text.chars().count();
    if width <= len {
        return text.chars().take(width).collect();
    }

    let left = (width - len) / 2;
    let right = width - len - left;
    format!("{}{}{}", " ".repeat(left), text, " ".repeat(right))
}

fn task_summary_current_branch(tasks: &[Task], branch: &str) -> (usize, usize, usize) {
    let today = Local::now().date_naive();
    let mut open = 0usize;
    let mut overdue = 0usize;
    let mut due_today = 0usize;

    for task in tasks.iter() {
        if task.done || task.archived {
            continue;
        }
        if !task.branch.eq_ignore_ascii_case(branch) {
            continue;
        }
        open += 1;
        if let Some(due) = task.due {
            if due < today {
                overdue += 1;
            } else if due == today {
                due_today += 1;
            }
        }
    }

    (open, overdue, due_today)
}

fn task_summary_all(tasks: &[Task]) -> (usize, usize, usize) {
    let today = Local::now().date_naive();
    let mut open = 0usize;
    let mut overdue = 0usize;
    let mut due_today = 0usize;

    for task in tasks.iter() {
        if task.done || task.archived {
            continue;
        }
        open += 1;
        if let Some(due) = task.due {
            if due < today {
                overdue += 1;
            } else if due == today {
                due_today += 1;
            }
        }
    }

    (open, overdue, due_today)
}

fn print_settings(state: &crate::model::AppState, color: bool) {
    let name = state
        .profile
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("friend (default)");
    let pronouns = state
        .profile
        .pronouns
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("<not set>");
    let message = state
        .profile
        .daily_message
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("<default>");
    let greeting = if state.profile.daily_greeting {
        "on"
    } else {
        "off"
    };
    let greeted = state
        .profile
        .last_greeted
        .map(|d| d.to_string())
        .unwrap_or_else(|| "<never>".to_string());
    let columns = if state.profile.list_columns.is_empty() {
        "<default>".to_string()
    } else {
        state
            .profile
            .list_columns
            .iter()
            .map(|c| format!("{c:?}").to_lowercase())
            .collect::<Vec<String>>()
            .join(",")
    };

    if color {
        println!("{} {}", "name:".dimmed(), name);
        println!("{} {}", "pronouns:".dimmed(), pronouns);
        println!("{} {}", "daily_greeting:".dimmed(), greeting);
        println!(
            "{} {}",
            "day_start_hour:".dimmed(),
            state.profile.day_start_hour
        );
        println!(
            "{} {:?}",
            "greeting_style:".dimmed(),
            state.profile.greeting_style
        );
        println!(
            "{} {}",
            "greeting_summary:".dimmed(),
            if state.profile.greeting_summary {
                "on"
            } else {
                "off"
            }
        );
        println!(
            "{} {:?}",
            "summary_scope:".dimmed(),
            state.profile.summary_scope
        );
        println!(
            "{} {:?}",
            "encouragement:".dimmed(),
            state.profile.encouragement_mode
        );
        println!("{} {:?}", "list_view:".dimmed(), state.profile.list_view);
        println!("{} {}", "list_columns:".dimmed(), columns);
        println!(
            "{} {}",
            "auto_pager:".dimmed(),
            if state.profile.auto_pager {
                "on"
            } else {
                "off"
            }
        );
        println!("{} {}", "daily_message:".dimmed(), message);
        println!("{} {}", "last_greeted:".dimmed(), greeted);
    } else {
        println!("name:          {name}");
        println!("pronouns:      {pronouns}");
        println!("daily_greeting:{:>4}", greeting);
        println!("day_start_hour:{:>4}", state.profile.day_start_hour);
        println!("greeting_style: {:?}", state.profile.greeting_style);
        println!(
            "greeting_summary:{:>3}",
            if state.profile.greeting_summary {
                "on"
            } else {
                "off"
            }
        );
        println!("summary_scope: {:?}", state.profile.summary_scope);
        println!("encouragement: {:?}", state.profile.encouragement_mode);
        println!("list_view:     {:?}", state.profile.list_view);
        println!("list_columns:  {columns}");
        println!(
            "auto_pager:    {}",
            if state.profile.auto_pager {
                "on"
            } else {
                "off"
            }
        );
        println!("daily_message: {message}");
        println!("last_greeted:  {greeted}");
    }
}
