use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub fn fingerprint(text: &str) -> String {
    let mut h = DefaultHasher::new();
    text.hash(&mut h);
    format!("{:016x}", h.finish())
}

pub fn should_nudge_after_debounce(idle_streak: i32) -> bool {
    idle_streak >= 2
}

/// True when the assistant output repeats either the previous nudge's output
/// or the one before it, catching both A->A and A->B->A loops.
pub fn is_repeating_output(fp: &str, last: Option<&str>, prev: Option<&str>) -> bool {
    last == Some(fp) || prev == Some(fp)
}
