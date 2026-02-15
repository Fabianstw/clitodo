use crate::branch::is_default_branch;
use crate::cli::ExportFormat;
use crate::display::task_lines_plain;
use crate::model::Task;

pub fn export_tasks(view: &[&Task], format: ExportFormat) {
    match format {
        ExportFormat::Json => {
            let data: Vec<&Task> = view.to_vec();
            let bytes = serde_json::to_vec_pretty(&data).expect("serialize tasks");
            println!("{}", String::from_utf8_lossy(&bytes));
        }
        ExportFormat::Markdown => {
            for task in view {
                let status = if task.done { "x" } else { " " };
                println!("- [{status}] {} (#{})", task.title, task.id);
                if !is_default_branch(&task.branch) {
                    println!("  - branch: {}", task.branch);
                }
                if let Some(due) = task.due {
                    println!("  - due: {due}");
                }
                if let Some(priority) = task.priority {
                    println!("  - priority: {priority:?}");
                }
                if let Some(repeat) = task.repeat {
                    println!("  - repeat: {repeat:?}");
                }
                if !task.tags.is_empty() {
                    println!("  - tags: {}", task.tags.join(", "));
                }
                if let Some(uid) = task.uid.as_deref() {
                    println!("  - uid: {uid}");
                }
                if task.archived {
                    println!("  - archived: true");
                }
                if let Some(content) = task.content.as_deref() {
                    println!("  - content: {content}");
                }
            }
        }
        ExportFormat::Text => {
            for task in view {
                for line in task_lines_plain(task) {
                    println!("{line}");
                }
            }
        }
    }
}
