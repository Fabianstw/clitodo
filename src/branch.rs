use crate::model::{AppState, DEFAULT_BRANCH, Task, default_branch};

pub fn normalize_branch(branch: Option<String>) -> Option<String> {
    branch.and_then(|b| {
        let trimmed = b.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

pub fn branch_matches(task: &Task, branch: Option<&str>) -> bool {
    branch.is_some_and(|value| task.branch.eq_ignore_ascii_case(value))
}

pub fn is_default_branch(branch: &str) -> bool {
    branch.eq_ignore_ascii_case(DEFAULT_BRANCH)
}

pub fn normalize_state(state: &mut AppState) {
    let current =
        normalize_branch(Some(state.current_branch.clone())).unwrap_or_else(default_branch);
    state.current_branch = current;
}

pub fn print_branch_list(state: &AppState, tasks: &[Task]) {
    let mut branches = collect_branches(tasks);
    order_branches(&mut branches, &state.current_branch);

    if branches.is_empty() {
        println!("Current branch: {}", state.current_branch);
        println!("Known branches: (none)");
        return;
    }

    println!("Current branch: {}", state.current_branch);
    println!("Known branches:");
    for branch in branches {
        if branch.eq_ignore_ascii_case(&state.current_branch) {
            println!("* {branch}");
        } else {
            println!("  {branch}");
        }
    }
}

pub fn collect_branches(tasks: &[Task]) -> Vec<String> {
    let mut branches: Vec<String> = tasks.iter().map(|t| t.branch.clone()).collect();
    branches.sort();
    branches.dedup();
    branches
}

pub fn order_branches(branches: &mut Vec<String>, current: &str) {
    branches.sort_by_key(|branch| branch.to_lowercase());
    if let Some(pos) = branches
        .iter()
        .position(|b| b.eq_ignore_ascii_case(current))
    {
        let current_branch = branches.remove(pos);
        branches.insert(0, current_branch);
    }
}
