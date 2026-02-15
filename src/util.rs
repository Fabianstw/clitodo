use chrono::{Local, Months, NaiveDate};
use std::io::{self, Write};

use crate::model::{Priority, Repeat};

pub fn parse_due(s: &str) -> Result<NaiveDate, String> {
    let s = s.trim().to_lowercase();
    let today = Local::now().date_naive();

    if s == "today" {
        return Ok(today);
    }
    if s == "tomorrow" {
        return Ok(today.succ_opt().ok_or("date overflow")?);
    }

    // YYYY-MM-DD
    if let Ok(d) = NaiveDate::parse_from_str(&s, "%Y-%m-%d") {
        return Ok(d);
    }

    // DDMMYYYY (e.g. 18022026)
    if s.len() == 8 && s.chars().all(|c| c.is_ascii_digit()) {
        return NaiveDate::parse_from_str(&s, "%d%m%Y")
            .map_err(|_| "expected DDMMYYYY like 18022026".to_string());
    }

    Err("expected today|tomorrow|YYYY-MM-DD|DDMMYYYY".into())
}

pub fn priority_rank(p: Option<Priority>) -> u8 {
    match p {
        Some(Priority::High) => 0,
        Some(Priority::Medium) => 1,
        Some(Priority::Low) => 2,
        None => 3,
    }
}

pub fn prompt_input(prompt: &str) -> String {
    print!("{prompt}");
    let _ = io::stdout().flush();

    let mut input = String::new();
    let _ = io::stdin().read_line(&mut input);
    input.trim_end().to_string()
}

pub fn normalize_tag(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_lowercase())
}

pub fn normalize_tags(values: &[String]) -> Vec<String> {
    let mut tags: Vec<String> = values.iter().filter_map(|t| normalize_tag(t)).collect();
    tags.sort();
    tags.dedup();
    tags
}

pub fn advance_due(due: NaiveDate, repeat: Repeat) -> Option<NaiveDate> {
    match repeat {
        Repeat::Daily => due.checked_add_days(chrono::Days::new(1)),
        Repeat::Weekly => due.checked_add_days(chrono::Days::new(7)),
        Repeat::Monthly => due.checked_add_months(Months::new(1)),
    }
}

pub fn parse_bool_flag(value: &str) -> Option<bool> {
    match value.trim().to_lowercase().as_str() {
        "true" | "1" | "yes" | "y" => Some(true),
        "false" | "0" | "no" | "n" => Some(false),
        _ => None,
    }
}
