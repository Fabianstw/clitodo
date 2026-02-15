use dirs::data_local_dir;
use std::{fs, path::PathBuf};

use crate::model::{AppState, Task};

fn base_dir() -> PathBuf {
    // /Users/<user>/Library/Application Support/todo/
    let mut base = data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    base.push("todo");
    fs::create_dir_all(&base).expect("Failed to create data dir");
    base
}

pub fn storage_path() -> PathBuf {
    let mut base = base_dir();
    base.push("tasks.json");
    base
}

pub fn state_path() -> PathBuf {
    let mut base = base_dir();
    base.push("state.json");
    base
}

pub fn load_tasks(path: &PathBuf) -> Vec<Task> {
    let Ok(bytes) = fs::read(path) else {
        return vec![];
    };
    serde_json::from_slice(&bytes).unwrap_or_else(|_| vec![])
}

pub fn save_tasks(path: &PathBuf, tasks: &Vec<Task>) {
    let bytes = serde_json::to_vec_pretty(tasks).expect("serialize tasks");
    fs::write(path, bytes).expect("write tasks");
}

pub fn load_state(path: &PathBuf) -> AppState {
    let Ok(bytes) = fs::read(path) else {
        return AppState::default();
    };
    serde_json::from_slice(&bytes).unwrap_or_else(|_| AppState::default())
}

pub fn save_state(path: &PathBuf, state: &AppState) {
    let bytes = serde_json::to_vec_pretty(state).expect("serialize state");
    fs::write(path, bytes).expect("write state");
}
