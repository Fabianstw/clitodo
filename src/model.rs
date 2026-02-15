use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

pub const DEFAULT_BRANCH: &str = "personal";

pub fn default_branch() -> String {
    DEFAULT_BRANCH.to_string()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppState {
    #[serde(default = "default_branch")]
    pub current_branch: String,
    #[serde(default)]
    pub config: AppConfig,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            current_branch: default_branch(),
            config: AppConfig::default(),
        }
    }
}

fn default_sort() -> SortKey {
    SortKey::Due
}

fn default_desc() -> bool {
    false
}

fn default_color() -> bool {
    true
}

fn default_reminder_days() -> u32 {
    0
}

fn default_id_scope() -> IdScope {
    IdScope::Global
}

fn default_use_uuid() -> bool {
    false
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_sort")]
    pub default_sort: SortKey,
    #[serde(default = "default_desc")]
    pub default_desc: bool,
    #[serde(default = "default_color")]
    pub color: bool,
    #[serde(default = "default_reminder_days")]
    pub reminder_days: u32,
    #[serde(default = "default_id_scope")]
    pub id_scope: IdScope,
    #[serde(default = "default_use_uuid")]
    pub use_uuid: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            default_sort: default_sort(),
            default_desc: default_desc(),
            color: default_color(),
            reminder_days: default_reminder_days(),
            id_scope: default_id_scope(),
            use_uuid: default_use_uuid(),
        }
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, clap::ValueEnum)]
pub enum SortKey {
    Due,
    Priority,
    Created,
    Id,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, clap::ValueEnum, PartialEq, Eq)]
pub enum IdScope {
    Global,
    Branch,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, clap::ValueEnum)]
pub enum Priority {
    Low,
    Medium,
    High,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, clap::ValueEnum)]
pub enum Repeat {
    Daily,
    Weekly,
    Monthly,
}

impl FromStr for Repeat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "daily" => Ok(Repeat::Daily),
            "weekly" => Ok(Repeat::Weekly),
            "monthly" => Ok(Repeat::Monthly),
            _ => Err("expected daily|weekly|monthly".to_string()),
        }
    }
}

impl FromStr for Priority {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "low" => Ok(Priority::Low),
            "medium" => Ok(Priority::Medium),
            "high" => Ok(Priority::High),
            _ => Err("expected low|medium|high".to_string()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    pub id: u64,
    #[serde(default)]
    pub uid: Option<String>,
    pub title: String,
    pub content: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub due: Option<NaiveDate>,
    pub priority: Option<Priority>,
    pub repeat: Option<Repeat>,
    #[serde(default = "default_branch")]
    pub branch: String,
    #[serde(default)]
    pub archived: bool,
    pub done: bool,
    pub created_at: String, // keep simple for v1
}
