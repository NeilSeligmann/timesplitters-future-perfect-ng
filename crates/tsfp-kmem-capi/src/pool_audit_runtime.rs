//! Process-local diagnostic shadow for the opt-in game-pool route.
//!
//! This records guest payload addresses and internal generations. The game ABI still passes raw
//! pointers, so a pointer alone cannot distinguish an old handle after its address is reused.
use std::sync::{Mutex, OnceLock};

use tsfp_engine_memory::pool_audit::{AuditVerdict, PoolAudit};

static AUDIT: OnceLock<Mutex<PoolAudit>> = OnceLock::new();
const MAX_RECORDED_VIOLATIONS: usize = 256;

fn with_audit<R>(f: impl FnOnce(&mut PoolAudit) -> R) -> R {
    let mutex = AUDIT.get_or_init(|| Mutex::new(PoolAudit::new()));
    let mut audit = mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    f(&mut audit)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_kmem_game_pool_audit_alloc(payload: u32, size: u32) -> u32 {
    with_audit(|audit| u32::from(audit.record_alloc(payload, size).is_none()))
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_kmem_game_pool_audit_free(payload: u32) -> u32 {
    with_audit(|audit| {
        if audit.violations().len() >= MAX_RECORDED_VIOLATIONS {
            return 1;
        }
        match audit.record_free(payload) {
            AuditVerdict::Live => 0,
            AuditVerdict::NotLive | AuditVerdict::StaleGeneration { .. } => 1,
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_kmem_game_pool_audit_reset() {
    with_audit(|audit| *audit = PoolAudit::new());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_observer_tracks_successful_pool_lifetimes_and_reset() {
        tsfp_kmem_game_pool_audit_reset();
        assert_eq!(tsfp_kmem_game_pool_audit_alloc(0, 0x40), 1);
        assert_eq!(tsfp_kmem_game_pool_audit_free(0), 0);
        assert_eq!(tsfp_kmem_game_pool_audit_alloc(0x1000, 0x40), 0);
        assert_eq!(tsfp_kmem_game_pool_audit_free(0x1000), 0);
        assert_eq!(tsfp_kmem_game_pool_audit_free(0x1000), 1);
        tsfp_kmem_game_pool_audit_reset();
        assert_eq!(tsfp_kmem_game_pool_audit_free(0x1000), 1);
    }
}
