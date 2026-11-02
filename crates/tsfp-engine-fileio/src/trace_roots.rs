//! Guest lifecycle entry points registered for file-I/O Tier 2 trace coverage.
//!
//! These addresses identify guest callers whose file-I/O effects cross the Rust
//! backend seam. Registering them makes the trace gate observe their execution;
//! it does not claim that the guest caller bodies themselves have been ported.
pub const ROOTS: [u32; 4] = [0x37db94, 0x380309, 0x46d7c4, 0x46d8f6];

#[cfg(test)]
mod tests {
    use super::ROOTS;

    #[test]
    fn lifecycle_trace_roots_cover_unregistered_teardown_and_mu_calls() {
        assert_eq!(ROOTS, [0x37db94, 0x380309, 0x46d7c4, 0x46d8f6]);
    }
}
