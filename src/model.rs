use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
    #[serde(default)]
    pub profile: UserProfile,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            current_branch: default_branch(),
            config: AppConfig::default(),
            profile: UserProfile::default(),
        }
    }
}

fn default_daily_greeting() -> bool {
    true
}

fn default_day_start_hour() -> u8 {
    6
}

fn default_greeting_summary() -> bool {
    true
}

fn default_greeting_style() -> GreetingStyle {
    GreetingStyle::Banner
}

fn default_summary_scope() -> SummaryScope {
    SummaryScope::Current
}

fn default_encouragement_mode() -> EncouragementMode {
    EncouragementMode::BuiltIn
}

fn default_auto_pager() -> bool {
    true
}

fn default_list_view() -> ListViewStyle {
    ListViewStyle::Table
}

pub fn default_list_columns() -> Vec<ListColumn> {
    vec![ListColumn::Due, ListColumn::Priority]
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, clap::ValueEnum)]
pub enum ListViewStyle {
    Table,
    Compact,
    Cards,
    Classic,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, clap::ValueEnum, PartialEq, Eq)]
pub enum ListColumn {
    Due,
    Priority,
    Branch,
    Tags,
    Repeat,
    Content,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, clap::ValueEnum)]
pub enum GreetingStyle {
    Banner,
    Compact,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, clap::ValueEnum)]
pub enum SummaryScope {
    Current,
    All,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, clap::ValueEnum)]
pub enum EncouragementMode {
    Off,
    BuiltIn,
    CustomOnly,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserProfile {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub pronouns: Option<String>,
    #[serde(default)]
    pub daily_message: Option<String>,
    #[serde(default = "default_daily_greeting")]
    pub daily_greeting: bool,
    #[serde(default = "default_day_start_hour")]
    pub day_start_hour: u8,
    #[serde(default = "default_greeting_style")]
    pub greeting_style: GreetingStyle,
    #[serde(default = "default_greeting_summary")]
    pub greeting_summary: bool,
    #[serde(default = "default_summary_scope")]
    pub summary_scope: SummaryScope,
    #[serde(default = "default_encouragement_mode")]
    pub encouragement_mode: EncouragementMode,

    // List UI preferences
    #[serde(default = "default_list_view")]
    pub list_view: ListViewStyle,
    #[serde(default = "default_list_columns")]
    pub list_columns: Vec<ListColumn>,
    #[serde(default = "default_auto_pager")]
    pub auto_pager: bool,
    /// User-defined saved commands (aliases). Key is the command name; value is argv tokens after `todo`.
    #[serde(default)]
    pub saved_commands: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub last_greeted: Option<NaiveDate>,
}

impl Default for UserProfile {
    fn default() -> Self {
        Self {
            name: None,
            pronouns: None,
            daily_message: None,
            daily_greeting: default_daily_greeting(),
            day_start_hour: default_day_start_hour(),
            greeting_style: default_greeting_style(),
            greeting_summary: default_greeting_summary(),
            summary_scope: default_summary_scope(),
            encouragement_mode: default_encouragement_mode(),
            list_view: default_list_view(),
            list_columns: default_list_columns(),
            auto_pager: default_auto_pager(),
            saved_commands: BTreeMap::new(),
            last_greeted: None,
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
