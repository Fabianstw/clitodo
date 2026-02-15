use std::cmp::Ordering;

use crate::branch::{collect_branches, is_default_branch, order_branches};
use crate::model::{AppState, Priority, SortKey, Task};
use crate::sort::sort_tasks;
use owo_colors::OwoColorize;

pub fn print_task_list(view: &[&Task], color: bool) {
    if view.is_empty() {
        println!("No tasks.");
        return;
    }

    for task in view {
        for line in task_lines(task, color) {
            println!("{line}");
        }
    }
}

pub fn print_task_list_grouped(
    tasks: &[Task],
    state: &AppState,
    all: bool,
    sort: SortKey,
    desc: bool,
    color: bool,
) {
    let mut branches = collect_branches(tasks);
    if branches.is_empty() {
        println!("No tasks.");
        return;
    }

    order_branches(&mut branches, &state.current_branch);

    for branch in branches {
        let mut view: Vec<&Task> = tasks
            .iter()
            .filter(|t| t.branch.eq_ignore_ascii_case(&branch))
            .filter(|t| all || !t.done)
            .collect();

        if view.is_empty() {
            continue;
        }

        sort_tasks(&mut view, sort, desc);
        println!("\n{}", format_header(&branch, color));
        for task in view {
            for line in task_lines(task, color) {
                println!("{line}");
            }
        }
    }
}

pub fn task_lines_plain(task: &Task) -> Vec<String> {
    task_lines(task, false)
}

fn task_lines(task: &Task, color: bool) -> Vec<String> {
    let status = format_status(task.done, color);
    let id = format_id(task.id, color);
    let title = format_title(&task.title, color);
    let mut lines = vec![format!("[{status}] {id}  {title}")];

    let mut meta_parts: Vec<String> = Vec::new();
    if !is_default_branch(&task.branch) {
        meta_parts.push(format!(
            "{} {}",
            format_label("branch:", color),
            task.branch
        ));
    }
    if task.archived {
        meta_parts.push(format!("{} {}", format_label("archived:", color), "yes"));
    }
    if let Some(due) = task.due {
        let due_text = format_due(due, task.done, color);
        meta_parts.push(format!("{} {due_text}", format_label("due:", color)));
    }
    if let Some(priority) = task.priority {
        let priority_text = format_priority(priority, color);
        meta_parts.push(format!(
            "{} {priority_text}",
            format_label("priority:", color)
        ));
    }
    if let Some(repeat) = task.repeat {
        meta_parts.push(format!("{} {repeat:?}", format_label("repeat:", color)));
    }
    if !task.tags.is_empty() {
        meta_parts.push(format!(
            "{} {}",
            format_label("tags:", color),
            task.tags.join(", ")
        ));
    }
    if let Some(content) = task.content.as_deref() {
        meta_parts.push(format!("{} {content}", format_label("content:", color)));
    }

    if !meta_parts.is_empty() {
        lines.push(format!("    {}", meta_parts.join(" | ")));
    }

    lines
}

pub fn print_task_view(task: &Task, color: bool) {
    println!(
        "{} {}",
        format_label("ID:", color),
        format_id(task.id, color)
    );
    println!(
        "{} {}",
        format_label("Title:", color),
        format_title(&task.title, color)
    );
    if !is_default_branch(&task.branch) {
        println!("{} {}", format_label("Branch:", color), task.branch);
    }
    if let Some(content) = task.content.as_deref() {
        println!("{} {content}", format_label("Content:", color));
    }
    if let Some(due) = task.due {
        println!(
            "{} {}",
            format_label("Due:", color),
            format_due(due, task.done, color)
        );
    }
    if let Some(priority) = task.priority {
        println!(
            "{} {}",
            format_label("Priority:", color),
            format_priority(priority, color)
        );
    }
    if let Some(repeat) = task.repeat {
        println!("{} {repeat:?}", format_label("Repeat:", color));
    }
    if !task.tags.is_empty() {
        println!("{} {}", format_label("Tags:", color), task.tags.join(", "));
    }
    if let Some(uid) = task.uid.as_deref() {
        println!("{} {uid}", format_label("UID:", color));
    }
    if task.archived {
        println!("{} yes", format_label("Archived:", color));
    }
    println!(
        "{} {}",
        format_label("Status:", color),
        format_status(task.done, color)
    );
    println!(
        "{} {}",
        format_label("Created:", color),
        format_created(&task.created_at, color)
    );
}

fn format_status(done: bool, color: bool) -> String {
    if done {
        if color {
            format!("{}", "done".green())
        } else {
            "done".to_string()
        }
    } else if color {
        format!("{}", "todo".yellow())
    } else {
        "todo".to_string()
    }
}

fn format_id(id: u64, color: bool) -> String {
    let value = format!("#{:>3}", id);
    if color {
        format!("{}", value.dimmed())
    } else {
        value
    }
}

fn format_title(title: &str, color: bool) -> String {
    if color {
        format!("{}", title.bold())
    } else {
        title.to_string()
    }
}

fn format_label(label: &str, color: bool) -> String {
    if color {
        format!("{}", label.dimmed())
    } else {
        label.to_string()
    }
}

fn format_due(due: chrono::NaiveDate, done: bool, color: bool) -> String {
    let text = due.to_string();
    if !color {
        return text;
    }

    if done {
        return format!("{}", text.dimmed());
    }

    let today = chrono::Local::now().date_naive();
    match due.cmp(&today) {
        Ordering::Less => format!("{}", text.red()),
        Ordering::Equal => format!("{}", text.yellow()),
        Ordering::Greater => format!("{}", text.cyan()),
    }
}

fn format_priority(priority: Priority, color: bool) -> String {
    let text = format!("{priority:?}");
    if !color {
        return text;
    }

    match priority {
        Priority::High => format!("{}", text.red()),
        Priority::Medium => format!("{}", text.yellow()),
        Priority::Low => format!("{}", text.green()),
    }
}

fn format_created(value: &str, color: bool) -> String {
    if color {
        format!("{}", value.dimmed())
    } else {
        value.to_string()
    }
}

fn format_header(branch: &str, color: bool) -> String {
    let text = format!("== {branch} ==");
    if color {
        format!("{}", text.bold())
    } else {
        text
    }
}
