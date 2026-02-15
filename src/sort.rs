use std::cmp::Ordering;

use crate::model::{Priority, SortKey, Task};
use crate::util::priority_rank;

pub fn sort_tasks(view: &mut Vec<&Task>, key: SortKey, desc: bool) {
    view.sort_by(|a, b| compare_tasks(a, b, key, desc));
}

fn compare_tasks(a: &Task, b: &Task, key: SortKey, desc: bool) -> Ordering {
    match key {
        SortKey::Due => compare_due(a, b, desc)
            .then_with(|| compare_priority(a, b, false))
            .then_with(|| a.id.cmp(&b.id)),
        SortKey::Priority => compare_priority(a, b, desc)
            .then_with(|| compare_due(a, b, false))
            .then_with(|| a.id.cmp(&b.id)),
        SortKey::Created => {
            let ord = a.created_at.cmp(&b.created_at);
            if desc { ord.reverse() } else { ord }
        }
        SortKey::Id => {
            let ord = a.id.cmp(&b.id);
            if desc { ord.reverse() } else { ord }
        }
    }
}

fn compare_due(a: &Task, b: &Task, desc: bool) -> Ordering {
    match (a.due, b.due) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(ad), Some(bd)) => {
            if desc {
                bd.cmp(&ad)
            } else {
                ad.cmp(&bd)
            }
        }
    }
}

fn compare_priority(a: &Task, b: &Task, desc: bool) -> Ordering {
    let ar = priority_sort_value(a.priority, desc);
    let br = priority_sort_value(b.priority, desc);
    ar.cmp(&br)
}

fn priority_sort_value(p: Option<Priority>, desc: bool) -> u8 {
    match p {
        Some(_) => {
            let rank = priority_rank(p);
            if desc { 2 - rank } else { rank }
        }
        None => 3,
    }
}
