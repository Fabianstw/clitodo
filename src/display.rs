use std::cmp::Ordering;
use std::io::{IsTerminal, Write};
use std::process::{Command, Stdio};

use crate::branch::{collect_branches, is_default_branch, order_branches};
use crate::model::{AppState, ListColumn, ListViewStyle, Priority, SortKey, Task};
use crate::sort::sort_tasks;
use owo_colors::OwoColorize;

pub fn print_task_list(view: &[&Task], state: &AppState, color: bool, group_by_day: bool) {
    if view.is_empty() {
        println!("No tasks.");
        return;
    }

    let text = render_task_list(view, state, color, group_by_day);
    output_text(&text, state.profile.auto_pager, color);
}

pub fn print_task_list_due_split(due: &[&Task], no_due: &[&Task], state: &AppState, color: bool) {
    if due.is_empty() && no_due.is_empty() {
        println!("No tasks.");
        return;
    }

    let text = render_task_list_due_split(due, no_due, state, color).join("\n");
    output_text(&text, state.profile.auto_pager, color);
}

pub fn print_task_list_grouped(
    tasks: &[Task],
    state: &AppState,
    all: bool,
    sort: SortKey,
    desc: bool,
    color: bool,
    group_by_day: bool,
) {
    let mut branches = collect_branches(tasks);
    if branches.is_empty() {
        println!("No tasks.");
        return;
    }

    order_branches(&mut branches, &state.current_branch);

    let mut lines: Vec<String> = Vec::new();
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
        lines.push("".to_string());
        lines.push(format_header(&branch, color));
        lines.extend(render_task_list_lines(&view, state, color, group_by_day));
    }

    if lines.iter().all(|l| l.trim().is_empty()) {
        println!("No tasks.");
        return;
    }

    let text = lines.join("\n");
    output_text(&text, state.profile.auto_pager, color);
}

pub fn task_lines_plain(task: &Task) -> Vec<String> {
    task_lines(task, false)
}

fn render_task_list(view: &[&Task], state: &AppState, color: bool, group_by_day: bool) -> String {
    render_task_list_lines(view, state, color, group_by_day).join("\n")
}

fn render_task_list_lines(
    view: &[&Task],
    state: &AppState,
    color: bool,
    group_by_day: bool,
) -> Vec<String> {
    match state.profile.list_view {
        ListViewStyle::Table => {
            if group_by_day {
                render_table_grouped_by_due_day(view, state, color)
            } else {
                render_table(view, state, color)
            }
        }
        ListViewStyle::Compact => render_compact(view, state, color),
        ListViewStyle::Cards => render_cards(view, state, color),
        ListViewStyle::Classic => render_classic(view, color),
    }
}

fn render_task_list_due_split(
    due: &[&Task],
    no_due: &[&Task],
    state: &AppState,
    color: bool,
) -> Vec<String> {
    match state.profile.list_view {
        ListViewStyle::Table => render_table_grouped_by_due_presence(due, no_due, state, color),
        _ => {
            let term_width = terminal_width().unwrap_or(80).clamp(60, 240);
            let mut lines: Vec<String> = Vec::new();

            if !due.is_empty() {
                lines.push(format_label_group_header(
                    "== Due date ==",
                    term_width,
                    color,
                ));
                lines.extend(render_task_list_lines(due, state, color, false));
            }

            if !no_due.is_empty() {
                if !lines.is_empty() {
                    lines.push(String::new());
                }
                lines.push(format_label_group_header(
                    "== No due date ==",
                    term_width,
                    color,
                ));
                lines.extend(render_task_list_lines(no_due, state, color, false));
            }

            lines
        }
    }
}

fn render_table_grouped_by_due_presence(
    due: &[&Task],
    no_due: &[&Task],
    state: &AppState,
    color: bool,
) -> Vec<String> {
    let layout = compute_table_layout(state);
    let term_width = layout.term_width;

    let mut lines: Vec<String> = Vec::new();
    lines.extend(build_table_header(
        &layout.cols,
        layout.title_width,
        layout.term_width,
        color,
    ));

    if !due.is_empty() {
        lines.push(format_label_group_header(
            "== Due date ==",
            term_width,
            color,
        ));
        lines.extend(render_table_rows_only(due, &layout, color));
    }

    if !no_due.is_empty() {
        if !due.is_empty() {
            lines.push(String::new());
        }
        lines.push(format_label_group_header(
            "== No due date ==",
            term_width,
            color,
        ));
        lines.extend(render_table_rows_only(no_due, &layout, color));
    }

    lines
}

fn render_table_grouped_by_due_day(view: &[&Task], state: &AppState, color: bool) -> Vec<String> {
    let layout = compute_table_layout(state);
    let term_width = layout.term_width;

    let mut open_dated: std::collections::BTreeMap<chrono::NaiveDate, Vec<&Task>> =
        std::collections::BTreeMap::new();
    let mut open_no_due: Vec<&Task> = Vec::new();
    let mut done_dated: std::collections::BTreeMap<chrono::NaiveDate, Vec<&Task>> =
        std::collections::BTreeMap::new();
    let mut done_no_due: Vec<&Task> = Vec::new();

    for task in view {
        let (dated, no_due) = if task.done {
            (&mut done_dated, &mut done_no_due)
        } else {
            (&mut open_dated, &mut open_no_due)
        };

        if let Some(d) = task.due {
            dated.entry(d).or_default().push(*task);
        } else {
            no_due.push(*task);
        }
    }

    let mut lines: Vec<String> = Vec::new();
    lines.extend(build_table_header(
        &layout.cols,
        layout.title_width,
        layout.term_width,
        color,
    ));

    let mut first_group = true;
    for (date, tasks) in open_dated.into_iter() {
        if !first_group {
            lines.push(String::new());
        }
        first_group = false;
        lines.push(format_date_group_header(Some(date), term_width, color));
        lines.extend(render_table_rows_only(&tasks, &layout, color));
    }
    if !open_no_due.is_empty() {
        if !first_group {
            lines.push(String::new());
        }
        first_group = false;
        lines.push(format_date_group_header(None, term_width, color));
        lines.extend(render_table_rows_only(&open_no_due, &layout, color));
    }

    if !(done_dated.is_empty() && done_no_due.is_empty()) {
        if !first_group {
            lines.push(String::new());
        }

        let mut first_done_group = true;
        for (date, tasks) in done_dated.into_iter() {
            if !first_done_group {
                lines.push(String::new());
            }
            first_done_group = false;
            lines.push(format_date_group_header(Some(date), term_width, color));
            lines.extend(render_table_rows_only(&tasks, &layout, color));
        }
        if !done_no_due.is_empty() {
            if !first_done_group {
                lines.push(String::new());
            }
            lines.push(format_date_group_header(None, term_width, color));
            lines.extend(render_table_rows_only(&done_no_due, &layout, color));
        }
    }
    lines
}

fn format_date_group_header(
    date: Option<chrono::NaiveDate>,
    table_width: usize,
    color: bool,
) -> String {
    let label = match date {
        Some(d) => format!("== {} ==", d),
        None => "== No due date ==".to_string(),
    };
    format_label_group_header(&label, table_width, color)
}

fn format_label_group_header(label: &str, table_width: usize, color: bool) -> String {
    let fill = table_width.saturating_sub(label.len());
    let line = format!("{}{}", label, "─".repeat(fill));
    if color {
        line.bright_blue().bold().to_string()
    } else {
        line
    }
}

fn render_classic(view: &[&Task], color: bool) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for task in view {
        lines.extend(task_lines(task, color));
    }
    lines
}

fn render_table(view: &[&Task], state: &AppState, color: bool) -> Vec<String> {
    let layout = compute_table_layout(state);
    let mut lines: Vec<String> = Vec::new();
    lines.extend(build_table_header(
        &layout.cols,
        layout.title_width,
        layout.term_width,
        color,
    ));
    lines.extend(render_table_rows_only(view, &layout, color));
    lines
}

#[derive(Debug, Clone)]
struct TableLayout {
    term_width: usize,
    cols: Vec<ListColumn>,
    title_width: usize,
    today: chrono::NaiveDate,
}

fn compute_table_layout(state: &AppState) -> TableLayout {
    let term_width = terminal_width().unwrap_or(80).clamp(60, 240);
    let cols = effective_columns(state);
    let cols = fit_columns(term_width, &cols);

    let today = chrono::Local::now().date_naive();

    let mut fixed = 0usize;
    fixed += 1; // status
    fixed += 1; // space
    fixed += 4; // id
    fixed += 2; // sep
    for c in cols.iter() {
        fixed += 2; // sep
        fixed += column_width(*c);
    }

    let title_width = term_width.saturating_sub(fixed).clamp(18, 120);

    TableLayout {
        term_width,
        cols,
        title_width,
        today,
    }
}

fn render_table_rows_only(view: &[&Task], layout: &TableLayout, color: bool) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for task in view {
        lines.push(build_table_row(
            task,
            &layout.cols,
            layout.title_width,
            layout.today,
            color,
        ));
    }
    lines
}

fn build_table_header(
    cols: &[ListColumn],
    title_width: usize,
    term_width: usize,
    color: bool,
) -> Vec<String> {
    let mut header = String::new();
    header.push(' ');
    header.push(' ');
    header.push_str(&pad_right("ID", 4));
    header.push_str("  ");
    header.push_str(&pad_right("Title", title_width));
    for c in cols.iter() {
        header.push_str("  ");
        header.push_str(&pad_right(column_header(*c), column_width(*c)));
    }

    let mut sep = String::new();
    let sep_len = std::cmp::min(term_width, header.chars().count());
    sep.push_str(&"─".repeat(sep_len));

    if color {
        vec![
            format!("{}", center_line(&header, term_width).bright_blue().bold()),
            sep.dimmed().to_string(),
        ]
    } else {
        vec![center_line(&header, term_width), sep]
    }
}

fn build_table_row(
    task: &Task,
    cols: &[ListColumn],
    title_width: usize,
    today: chrono::NaiveDate,
    color: bool,
) -> String {
    let status = if task.done { "✔" } else { "•" };
    let status = if !color {
        status.to_string()
    } else if task.done {
        format!("{}", status.green().bold())
    } else {
        format!("{}", status.yellow().bold())
    };

    let id_plain = format!("#{:>3}", task.id);
    let id_cell = pad_right(&id_plain, 4);
    let id_cell = if color {
        format!("{}", id_cell.dimmed())
    } else {
        id_cell
    };

    let title_plain = truncate_to_width(&task.title, title_width);
    let title_cell = pad_right(&title_plain, title_width);
    let title_cell = if !color {
        title_cell
    } else if task.done {
        format!("{}", title_cell.dimmed())
    } else {
        format!("{}", title_cell.bold())
    };

    let mut row = String::new();
    row.push_str(&status);
    row.push(' ');
    row.push_str(&id_cell);
    row.push_str("  ");
    row.push_str(&title_cell);

    for c in cols.iter() {
        row.push_str("  ");
        row.push_str(&format_column_cell(*c, task, today, color));
    }

    row
}

fn render_cards(view: &[&Task], state: &AppState, color: bool) -> Vec<String> {
    let term_width = terminal_width().unwrap_or(80).clamp(60, 240);
    let cols = effective_columns(state);
    let today = chrono::Local::now().date_naive();

    let mut lines: Vec<String> = Vec::new();
    for (idx, task) in view.iter().enumerate() {
        if idx > 0 {
            lines.push("".to_string());
        }

        let status = if task.done { "✔" } else { "•" };
        let status = if !color {
            status.to_string()
        } else if task.done {
            format!("{}", status.green().bold())
        } else {
            format!("{}", status.yellow().bold())
        };

        let id = format!("#{:>3}", task.id);
        let id = if color {
            format!("{}", id.dimmed())
        } else {
            id
        };

        let title_width = term_width.saturating_sub(10).clamp(18, 200);
        let title = truncate_to_width(&task.title, title_width);
        let title = if !color {
            title
        } else if task.done {
            format!("{}", title.dimmed())
        } else {
            format!("{}", title.bold())
        };

        lines.push(format!("{status} {id}  {title}"));

        let meta = build_meta_line(task, &cols, today);
        if !meta.is_empty() {
            let meta = truncate_to_width(&meta, term_width.saturating_sub(4));
            let meta = if color {
                format!("{}", meta.dimmed())
            } else {
                meta
            };
            lines.push(format!("    {meta}"));
        }
    }

    lines
}

fn render_compact(view: &[&Task], state: &AppState, color: bool) -> Vec<String> {
    let term_width = terminal_width().unwrap_or(80).clamp(60, 240);
    let cols = effective_columns(state);
    let today = chrono::Local::now().date_naive();

    let mut lines: Vec<String> = Vec::new();
    for task in view {
        let status = if task.done { "✔" } else { "•" };
        let status = if !color {
            status.to_string()
        } else if task.done {
            format!("{}", status.green().bold())
        } else {
            format!("{}", status.yellow().bold())
        };

        let id = format!("#{:>3}", task.id);
        let id = if color {
            format!("{}", id.dimmed())
        } else {
            id
        };

        let meta = build_meta_line(task, &cols, today);
        let mut line = if meta.is_empty() {
            format!("{status} {id}  {}", task.title)
        } else {
            format!("{status} {id}  {}  —  {meta}", task.title)
        };

        line = truncate_to_width(&line, term_width);
        lines.push(line);
    }
    lines
}

fn build_meta_line(task: &Task, cols: &[ListColumn], today: chrono::NaiveDate) -> String {
    let mut parts: Vec<String> = Vec::new();
    for c in cols.iter() {
        match c {
            ListColumn::Due => {
                if let Some(due) = task.due {
                    let label = if due < today {
                        "overdue"
                    } else if due == today {
                        "due"
                    } else {
                        "due"
                    };
                    parts.push(format!("{label}: {due}"));
                }
            }
            ListColumn::Priority => {
                if let Some(p) = task.priority {
                    parts.push(format!("p: {p:?}"));
                }
            }
            ListColumn::Branch => {
                if !is_default_branch(&task.branch) {
                    parts.push(format!("branch: {}", task.branch));
                }
            }
            ListColumn::Tags => {
                if !task.tags.is_empty() {
                    parts.push(format!("tags: {}", task.tags.join(",")));
                }
            }
            ListColumn::Repeat => {
                if let Some(r) = task.repeat {
                    parts.push(format!("repeat: {r:?}"));
                }
            }
            ListColumn::Content => {
                if let Some(c) = task.content.as_deref() {
                    let c = c.replace('\n', " ");
                    parts.push(format!("note: {c}"));
                }
            }
        }
    }

    parts.join(" · ")
}

fn effective_columns(state: &AppState) -> Vec<ListColumn> {
    if state.profile.list_columns.is_empty() {
        crate::model::default_list_columns()
    } else {
        state.profile.list_columns.clone()
    }
}

fn fit_columns(term_width: usize, cols: &[ListColumn]) -> Vec<ListColumn> {
    let mut selected = cols.to_vec();
    // Try to keep at least 18 chars for title.
    let min_title = 18usize;
    loop {
        let mut fixed = 0usize;
        fixed += 1 + 1 + 4 + 2; // status + space + id + sep
        for c in selected.iter() {
            fixed += 2 + column_width(*c);
        }
        if term_width.saturating_sub(fixed) >= min_title {
            return selected;
        }
        if selected.is_empty() {
            return selected;
        }
        selected.pop();
    }
}

fn column_width(c: ListColumn) -> usize {
    match c {
        ListColumn::Due => 10,
        ListColumn::Priority => 1,
        ListColumn::Branch => 12,
        ListColumn::Tags => 18,
        ListColumn::Repeat => 7,
        ListColumn::Content => 20,
    }
}

fn column_header(c: ListColumn) -> &'static str {
    match c {
        ListColumn::Due => "Due",
        ListColumn::Priority => "P",
        ListColumn::Branch => "Branch",
        ListColumn::Tags => "Tags",
        ListColumn::Repeat => "Repeat",
        ListColumn::Content => "Note",
    }
}

fn format_column_cell(c: ListColumn, task: &Task, today: chrono::NaiveDate, color: bool) -> String {
    let width = column_width(c);
    match c {
        ListColumn::Due => {
            let plain = task.due.map(|d| d.to_string()).unwrap_or_default();
            let cell = pad_right(&plain, width);
            if !color {
                return cell;
            }
            if task.done {
                return format!("{}", cell.dimmed());
            }
            if let Some(due) = task.due {
                match due.cmp(&today) {
                    Ordering::Less => format!("{}", cell.red().bold()),
                    Ordering::Equal => format!("{}", cell.yellow().bold()),
                    Ordering::Greater => format!("{}", cell.cyan()),
                }
            } else {
                cell
            }
        }
        ListColumn::Priority => {
            let plain = task
                .priority
                .map(|p| match p {
                    Priority::High => "H",
                    Priority::Medium => "M",
                    Priority::Low => "L",
                })
                .unwrap_or(" ");
            let cell = pad_right(plain, width);
            if !color {
                return cell;
            }
            match task.priority {
                Some(Priority::High) => format!("{}", cell.red().bold()),
                Some(Priority::Medium) => format!("{}", cell.yellow().bold()),
                Some(Priority::Low) => format!("{}", cell.green().bold()),
                None => cell,
            }
        }
        ListColumn::Branch => {
            let plain = truncate_to_width(&task.branch, width);
            let cell = pad_right(&plain, width);
            if color {
                format!("{}", cell.dimmed())
            } else {
                cell
            }
        }
        ListColumn::Tags => {
            let plain = truncate_to_width(&task.tags.join(","), width);
            let cell = pad_right(&plain, width);
            if color {
                format!("{}", cell.dimmed())
            } else {
                cell
            }
        }
        ListColumn::Repeat => {
            let plain = task
                .repeat
                .map(|r| format!("{r:?}").to_lowercase())
                .unwrap_or_default();
            let plain = truncate_to_width(&plain, width);
            let cell = pad_right(&plain, width);
            if color {
                format!("{}", cell.dimmed())
            } else {
                cell
            }
        }
        ListColumn::Content => {
            let plain = task.content.as_deref().unwrap_or("").replace('\n', " ");
            let plain = truncate_to_width(&plain, width);
            let cell = pad_right(&plain, width);
            if color {
                format!("{}", cell.dimmed())
            } else {
                cell
            }
        }
    }
}

fn output_text(text: &str, auto_pager: bool, color: bool) {
    if !auto_pager || !std::io::stdout().is_terminal() || std::env::var_os("NO_PAGER").is_some() {
        print!("{text}");
        if !text.ends_with('\n') {
            println!();
        }
        return;
    }

    let height = terminal_height().unwrap_or(24).clamp(10, 120);
    let lines = text.lines().count();
    if lines <= height {
        print!("{text}");
        if !text.ends_with('\n') {
            println!();
        }
        return;
    }

    let pager = std::env::var("PAGER").ok().filter(|s| !s.trim().is_empty());
    let (cmd, args): (String, Vec<&str>) = if let Some(pager) = pager {
        (pager, Vec::new())
    } else {
        (
            "less".to_string(),
            if color {
                vec!["-R", "-F", "-S", "-X"]
            } else {
                vec!["-F", "-S", "-X"]
            },
        )
    };

    let child = Command::new(&cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn();

    let Ok(mut child) = child else {
        print!("{text}");
        if !text.ends_with('\n') {
            println!();
        }
        return;
    };

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(text.as_bytes());
    }
    let _ = child.wait();
}

fn terminal_width() -> Option<usize> {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|w| *w > 0)
}

fn terminal_height() -> Option<usize> {
    std::env::var("LINES")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|h| *h > 0)
}

fn truncate_to_width(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let len = text.chars().count();
    if len <= width {
        return text.to_string();
    }

    if width == 1 {
        return "…".to_string();
    }

    let mut out: String = text.chars().take(width - 1).collect();
    out.push('…');
    out
}

fn pad_right(text: &str, width: usize) -> String {
    let len = text.chars().count();
    if len >= width {
        return text.chars().take(width).collect();
    }
    format!("{}{}", text, " ".repeat(width - len))
}

fn center_line(text: &str, width: usize) -> String {
    let len = text.chars().count();
    if width <= len {
        return text.to_string();
    }
    let pad = (width - len) / 2;
    format!("{}{}", " ".repeat(pad), text)
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
