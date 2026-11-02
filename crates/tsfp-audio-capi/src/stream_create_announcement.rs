//! Rust-owned once-per-reset stream Create announcement state.

use std::sync::atomic::{AtomicBool, Ordering};

static ANNOUNCED: AtomicBool = AtomicBool::new(false);

pub(crate) fn claim() -> bool {
    !ANNOUNCED.swap(true, Ordering::AcqRel)
}

pub(crate) fn reset() {
    ANNOUNCED.store(false, Ordering::Release);
}

#[cfg(test)]
mod tests {
    use super::{claim, reset};

    #[test]
    fn announcement_is_claimed_once_until_stream_reset() {
        reset();
        assert!(claim());
        assert!(!claim());
        reset();
        assert!(claim());
        reset();
    }
}
