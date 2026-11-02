//! Private opcode counters shared by the two audio log replay reports.

/// Increment the first matching entry, or append a new opcode in first-seen order.
/// Preserve the replay reports' existing usize addition and duplicate-key policy.
pub(crate) fn bump(v: &mut Vec<(String, usize)>, k: &str) {
    match v.iter_mut().find(|e| e.0 == k) {
        Some(e) => e.1 += 1,
        None => v.push((k.to_string(), 1)),
    }
}
