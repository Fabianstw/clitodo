use crate::model::{DEFAULT_BRANCH, Priority, Repeat, Task, default_branch};
use crate::util::normalize_tags;
use crate::util::{parse_due, prompt_input};

pub fn edit_interactive(task: &mut Task) {
    let title = prompt_input(&format!("Title [{}]: ", task.title));
    if !title.trim().is_empty() {
        task.title = title.trim().to_string();
    }

    let current_branch = if task.branch.trim().is_empty() {
        DEFAULT_BRANCH
    } else {
        task.branch.as_str()
    };
    let branch = prompt_input(&format!(
        "Branch [{}] (empty keep, '-' reset): ",
        current_branch
    ));
    let trimmed_branch = branch.trim();
    if !trimmed_branch.is_empty() {
        if trimmed_branch == "-" {
            task.branch = default_branch();
        } else {
            task.branch = trimmed_branch.to_string();
        }
    }

    let current = task.content.as_deref().unwrap_or("-");
    let input = prompt_input(&format!("Content [{}] (empty keep, '-' clear): ", current));
    let trimmed = input.trim();
    if !trimmed.is_empty() {
        if trimmed == "-" {
            task.content = None;
        } else {
            task.content = Some(trimmed.to_string());
        }
    }

    let current_tags = if task.tags.is_empty() {
        "-".to_string()
    } else {
        task.tags.join(", ")
    };
    let tags_input = prompt_input(&format!(
        "Tags [{}] (comma list, empty keep, '-' clear): ",
        current_tags
    ));
    let trimmed_tags = tags_input.trim();
    if !trimmed_tags.is_empty() {
        if trimmed_tags == "-" {
            task.tags.clear();
        } else {
            let parts: Vec<String> = trimmed_tags
                .split(',')
                .map(|t| t.trim().to_string())
                .collect();
            task.tags = normalize_tags(&parts);
        }
    }

    loop {
        let current = task
            .due
            .map(|d| d.to_string())
            .unwrap_or_else(|| "-".into());
        let input = prompt_input(&format!("Due [{}] (empty keep, '-' clear): ", current));
        let trimmed = input.trim();
        if trimmed.is_empty() {
            break;
        }
        if trimmed == "-" {
            task.due = None;
            break;
        }
        match parse_due(trimmed) {
            Ok(due) => {
                task.due = Some(due);
                break;
            }
            Err(e) => {
                eprintln!("Invalid due date: {e}");
            }
        }
    }

    loop {
        let current = task
            .priority
            .map(|p| format!("{p:?}"))
            .unwrap_or_else(|| "-".into());
        let input = prompt_input(&format!(
            "Priority [{}] (low|medium|high, empty keep, '-' clear): ",
            current
        ));
        let trimmed = input.trim();
        if trimmed.is_empty() {
            break;
        }
        if trimmed == "-" {
            task.priority = None;
            break;
        }
        match trimmed.parse::<Priority>() {
            Ok(priority) => {
                task.priority = Some(priority);
                break;
            }
            Err(e) => {
                eprintln!("Invalid priority: {e}");
            }
        }
    }

    loop {
        let current = task
            .repeat
            .map(|r| format!("{r:?}"))
            .unwrap_or_else(|| "-".into());
        let input = prompt_input(&format!(
            "Repeat [{}] (daily|weekly|monthly, empty keep, '-' clear): ",
            current
        ));
        let trimmed = input.trim();
        if trimmed.is_empty() {
            break;
        }
        if trimmed == "-" {
            task.repeat = None;
            break;
        }
        match trimmed.parse::<Repeat>() {
            Ok(repeat) => {
                task.repeat = Some(repeat);
                break;
            }
            Err(e) => {
                eprintln!("Invalid repeat: {e}");
            }
        }
    }
}
