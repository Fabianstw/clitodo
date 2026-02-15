use clap::{Parser, Subcommand, ValueEnum};

use crate::model::{IdScope, Priority, Repeat, SortKey};

#[derive(Parser)]
#[command(name = "todo", version, about = "Terminal todo app")]
pub struct Cli {
    /// Disable colored output
    #[arg(long = "no-color")]
    pub no_color: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Show or switch the current branch
    #[command(aliases = ["b", "switch"])]
    Branch {
        /// Branch name to switch to
        name: Option<String>,

        /// List known branches
        #[arg(short = 'l', long = "list")]
        list: bool,
    },

    /// Delete a branch
    #[command(aliases = ["branch-delete", "br-del"])]
    BranchDelete {
        /// Branch name to delete
        name: String,

        /// Delete tasks in the branch
        #[arg(short = 'f', long = "force")]
        force: bool,
    },

    /// Move all tasks from one branch to another
    #[command(aliases = ["branch-move", "br-mv"])]
    BranchMove { from: String, to: String },

    /// Duplicate tasks from one branch to another
    #[command(aliases = ["branch-dup", "br-cp"])]
    BranchDuplicate { from: String, to: String },

    /// Merge a branch into another
    #[command(aliases = ["branch-merge", "br-merge"])]
    BranchMerge { from: String, to: String },

    /// Create a new task
    #[command(aliases = ["c", "add"])]
    Create {
        /// Task title, e.g. "Go shopping"
        title: String,

        /// Due date: "tomorrow", "today", "YYYY-MM-DD", or "DDMMYYYY" (e.g. 18022026)
        #[arg(short = 'd', long = "due")]
        due: Option<String>,

        /// Priority
        #[arg(short = 'p', long = "priority")]
        priority: Option<Priority>,

        /// Optional task content, e.g. items to buy
        #[arg(short = 'c', long = "content")]
        content: Option<String>,

        /// Tag(s)
        #[arg(long = "tag")]
        tags: Vec<String>,

        /// Repeat interval
        #[arg(long = "repeat", value_enum)]
        repeat: Option<Repeat>,

        /// Branch/tab name (default: personal)
        #[arg(short = 'b', long = "branch")]
        branch: Option<String>,
    },

    /// Edit an existing task (interactive if no fields are provided)
    #[command(alias = "e")]
    Edit {
        id: u64,

        /// New title
        #[arg(long = "title")]
        title: Option<String>,

        /// New content
        #[arg(short = 'c', long = "content")]
        content: Option<String>,

        /// Add tag(s)
        #[arg(long = "tag")]
        tags: Vec<String>,

        /// Remove tag(s)
        #[arg(long = "remove-tag")]
        remove_tags: Vec<String>,

        /// New due date
        #[arg(short = 'd', long = "due")]
        due: Option<String>,

        /// New priority
        #[arg(short = 'p', long = "priority")]
        priority: Option<Priority>,

        /// New repeat interval
        #[arg(long = "repeat", value_enum)]
        repeat: Option<Repeat>,

        /// Clear content
        #[arg(long = "clear-content")]
        clear_content: bool,

        /// Clear all tags
        #[arg(long = "clear-tags")]
        clear_tags: bool,

        /// Clear due date
        #[arg(long = "clear-due")]
        clear_due: bool,

        /// Clear priority
        #[arg(long = "clear-priority")]
        clear_priority: bool,

        /// Clear repeat interval
        #[arg(long = "clear-repeat")]
        clear_repeat: bool,

        /// New branch
        #[arg(short = 'b', long = "branch")]
        branch: Option<String>,
    },

    /// List open tasks
    #[command(aliases = ["l", "ls"])]
    List {
        /// Include completed tasks
        #[arg(short = 'a', long = "all")]
        all: bool,

        /// Include archived tasks
        #[arg(long = "archived")]
        archived: bool,

        /// Filter by branch
        #[arg(short = 'b', long = "branch")]
        branch: Option<String>,

        /// Filter by tag (can repeat)
        #[arg(long = "tag")]
        tags: Vec<String>,

        /// Sort by: due, priority, created, id
        #[arg(short = 's', long = "sort", value_enum)]
        sort: Option<SortKey>,

        /// Sort descending
        #[arg(long = "desc")]
        desc: bool,

        /// Sort ascending
        #[arg(long = "asc")]
        asc: bool,
    },

    /// List tasks grouped by branch
    #[command(aliases = ["la", "list-all"])]
    ListAll {
        /// Include completed tasks
        #[arg(short = 'a', long = "all")]
        all: bool,

        /// Include archived tasks
        #[arg(long = "archived")]
        archived: bool,

        /// Sort by: due, priority, created, id
        #[arg(short = 's', long = "sort", value_enum)]
        sort: Option<SortKey>,

        /// Sort descending
        #[arg(long = "desc")]
        desc: bool,

        /// Filter by tag (can repeat)
        #[arg(long = "tag")]
        tags: Vec<String>,

        /// Sort ascending
        #[arg(long = "asc")]
        asc: bool,
    },

    /// List repeating tasks
    #[command(aliases = ["lr", "list-repeat"])]
    ListRepeat {
        /// Include completed tasks
        #[arg(short = 'a', long = "all")]
        all: bool,

        /// Include archived tasks
        #[arg(long = "archived")]
        archived: bool,

        /// Filter by branch
        #[arg(short = 'b', long = "branch")]
        branch: Option<String>,

        /// Filter by tag (can repeat)
        #[arg(long = "tag")]
        tags: Vec<String>,

        /// Sort by: due, priority, created, id
        #[arg(short = 's', long = "sort", value_enum)]
        sort: Option<SortKey>,

        /// Sort descending
        #[arg(long = "desc")]
        desc: bool,

        /// Sort ascending
        #[arg(long = "asc")]
        asc: bool,
    },

    /// List only completed tasks
    #[command(aliases = ["ld", "done-list"])]
    ListDone {
        /// Sort by: due, priority, created, id
        #[arg(short = 's', long = "sort", value_enum)]
        sort: Option<SortKey>,

        /// Sort descending
        #[arg(long = "desc")]
        desc: bool,

        /// Sort ascending
        #[arg(long = "asc")]
        asc: bool,

        /// Include archived tasks
        #[arg(long = "archived")]
        archived: bool,

        /// Filter by branch
        #[arg(short = 'b', long = "branch")]
        branch: Option<String>,

        /// Filter by tag (can repeat)
        #[arg(long = "tag")]
        tags: Vec<String>,
    },

    /// Search tasks by text (title or content)
    #[command(aliases = ["s", "find"])]
    Search {
        query: String,

        /// Include completed tasks
        #[arg(short = 'a', long = "all")]
        all: bool,

        /// Include archived tasks
        #[arg(long = "archived")]
        archived: bool,

        /// Sort by: due, priority, created, id
        #[arg(short = 's', long = "sort", value_enum)]
        sort: Option<SortKey>,

        /// Sort descending
        #[arg(long = "desc")]
        desc: bool,

        /// Sort ascending
        #[arg(long = "asc")]
        asc: bool,

        /// Filter by branch
        #[arg(short = 'b', long = "branch")]
        branch: Option<String>,

        /// Filter by tag (can repeat)
        #[arg(long = "tag")]
        tags: Vec<String>,
    },

    /// Show reminders for today and overdue tasks
    #[command(aliases = ["r", "remind", "due"])]
    Reminders {
        /// Filter by branch
        #[arg(short = 'b', long = "branch")]
        branch: Option<String>,

        /// Filter by tag (can repeat)
        #[arg(long = "tag")]
        tags: Vec<String>,
    },

    /// Show stats across tasks
    #[command(aliases = ["st", "stats"])]
    Stats,

    /// Mark matching tasks as done
    #[command(aliases = ["bd", "bulk-done"])]
    BulkDone {
        query: String,

        /// Filter by branch
        #[arg(short = 'b', long = "branch")]
        branch: Option<String>,
    },

    /// Mark matching tasks as not done
    #[command(aliases = ["bu", "bulk-undone"])]
    BulkUndone {
        query: String,

        /// Filter by branch
        #[arg(short = 'b', long = "branch")]
        branch: Option<String>,
    },

    /// Edit matching tasks in bulk
    #[command(aliases = ["be", "bulk-edit"])]
    BulkEdit {
        query: String,

        /// New due date
        #[arg(short = 'd', long = "due")]
        due: Option<String>,

        /// New priority
        #[arg(short = 'p', long = "priority")]
        priority: Option<Priority>,

        /// New repeat interval
        #[arg(long = "repeat", value_enum)]
        repeat: Option<Repeat>,

        /// Clear due date
        #[arg(long = "clear-due")]
        clear_due: bool,

        /// Clear priority
        #[arg(long = "clear-priority")]
        clear_priority: bool,

        /// Clear repeat interval
        #[arg(long = "clear-repeat")]
        clear_repeat: bool,

        /// Filter by branch
        #[arg(short = 'b', long = "branch")]
        branch: Option<String>,

        /// Add tag(s)
        #[arg(long = "tag")]
        tags: Vec<String>,

        /// Remove tag(s)
        #[arg(long = "remove-tag")]
        remove_tags: Vec<String>,

        /// Clear all tags
        #[arg(long = "clear-tags")]
        clear_tags: bool,
    },

    /// Delete matching tasks
    #[command(aliases = ["bx", "bulk-delete"])]
    BulkDelete {
        query: String,

        /// Filter by branch
        #[arg(short = 'b', long = "branch")]
        branch: Option<String>,
    },

    /// Move matching tasks to another branch
    #[command(aliases = ["bm", "bulk-move"])]
    BulkMove {
        query: String,

        /// Target branch
        to: String,

        /// Filter by branch
        #[arg(short = 'b', long = "branch")]
        branch: Option<String>,
    },

    /// Archive done tasks
    #[command(aliases = ["arc", "archive-done"])]
    Archive {
        /// Archive a single task by id
        id: Option<u64>,

        /// Archive done tasks in all branches
        #[arg(long = "all-branches")]
        all_branches: bool,

        /// Filter by branch
        #[arg(short = 'b', long = "branch")]
        branch: Option<String>,
    },

    /// Unarchive a task by id
    #[command(aliases = ["unarc", "unarchive"])]
    Unarchive { id: u64 },

    /// Mark a task as done by id
    #[command(alias = "d")]
    Done { id: u64 },

    /// Mark a task as not done by id
    #[command(alias = "u")]
    Undone { id: u64 },

    /// Toggle a task's done status by id
    #[command(alias = "t")]
    Toggle { id: u64 },

    /// Delete specified task by id
    #[command(aliases = ["x", "rm", "del"])]
    Delete { id: u64 },

    /// Clear all completed tasks
    #[command(alias = "clr")]
    Clear,

    /// View a specific task by id
    #[command(alias = "v")]
    View { id: u64 },

    /// Export tasks
    #[command(aliases = ["ex", "out"])]
    Export {
        /// Export format
        #[arg(value_enum, default_value = "json")]
        format: ExportFormat,

        /// Include completed tasks
        #[arg(short = 'a', long = "all")]
        all: bool,

        /// Include archived tasks
        #[arg(long = "archived")]
        archived: bool,

        /// Filter by branch
        #[arg(short = 'b', long = "branch")]
        branch: Option<String>,

        /// Filter by tag (can repeat)
        #[arg(long = "tag")]
        tags: Vec<String>,
    },

    /// Import tasks from a file
    #[command(aliases = ["imp", "in"])]
    Import {
        /// Import format
        #[arg(value_enum)]
        format: ImportFormat,

        /// Input file path
        #[arg(short = 'f', long = "file")]
        file: String,

        /// Override branch for imported tasks
        #[arg(short = 'b', long = "branch")]
        branch: Option<String>,
    },

    /// Show or update config defaults
    #[command(aliases = ["cfg", "config"])]
    Config {
        /// Default sort key
        #[arg(long = "default-sort", value_enum)]
        default_sort: Option<SortKey>,

        /// Default descending sort
        #[arg(long = "default-desc")]
        default_desc: Option<bool>,

        /// Default color output
        #[arg(long = "color")]
        color: Option<bool>,

        /// Default reminder window in days
        #[arg(long = "reminder-days")]
        reminder_days: Option<u32>,

        /// Task id scope
        #[arg(long = "id-scope", value_enum)]
        id_scope: Option<IdScope>,

        /// Generate UUIDs for tasks
        #[arg(long = "use-uuid")]
        use_uuid: Option<bool>,
    },

    /// Generate shell completions
    #[command(aliases = ["comp", "completion", "completions"])]
    Completions {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum ExportFormat {
    #[value(alias = "raw")]
    Text,
    Json,
    Markdown,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum ImportFormat {
    Json,
    Csv,
}
