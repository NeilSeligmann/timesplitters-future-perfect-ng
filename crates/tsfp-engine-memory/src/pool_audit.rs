//! Diagnostic shadow of live pool payloads (T2048, safety net, not a game port).
//!
//! The original pool (`0x3EA00` family, see `pool.rs`) keeps no allocation identity: `0x3EA40`
//! accepts any payload whose block header says "occupied", so a stale pointer to an address that
//! was freed and reused is silently accepted (REA T2365, `docs/t-memory-replace.md`, "duplicate
//! pool release versus stale address reuse"). `PoolAudit` observes allocations and frees from the
//! outside and reports what the original cannot: frees of payloads that are not live, and handles
//! that outlived their allocation. It never changes allocator behaviour or guest memory.
//!
//! Evidence class: unit-tested model only. It is not attached to a live host path yet.
use std::collections::BTreeMap;

/// One allocation's identity: address plus a generation that changes on every reuse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoolHandle {
    pub payload: u32,
    pub generation: u64,
}

/// What the audit concluded about a free or a handle check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditVerdict {
    /// The payload (or handle) matches a live allocation.
    Live,
    /// The payload was never allocated, or was already freed (double or foreign free).
    NotLive,
    /// The address is live again, but under a newer generation than the handle carries.
    StaleGeneration { current: u64 },
}

#[derive(Debug, Default)]
pub struct PoolAudit {
    live: BTreeMap<u32, (u64, u32)>,
    next_generation: u64,
    violations: Vec<(AuditVerdict, u32)>,
}

impl PoolAudit {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a successful allocation of `size` bytes at `payload`; null (failed) is ignored.
    /// Returns the handle a consumer can keep to detect later reuse.
    pub fn record_alloc(&mut self, payload: u32, size: u32) -> Option<PoolHandle> {
        if payload == 0 {
            return None;
        }
        self.next_generation += 1;
        let generation = self.next_generation;
        self.live.insert(payload, (generation, size));
        Some(PoolHandle {
            payload,
            generation,
        })
    }

    /// Record a free. Null is tolerated like `0x3EAD0`; anything not live is a violation.
    pub fn record_free(&mut self, payload: u32) -> AuditVerdict {
        if payload == 0 {
            return AuditVerdict::Live;
        }
        if self.live.remove(&payload).is_some() {
            AuditVerdict::Live
        } else {
            self.violations.push((AuditVerdict::NotLive, payload));
            AuditVerdict::NotLive
        }
    }

    /// Check a previously issued handle against the current live set.
    pub fn check(&mut self, handle: PoolHandle) -> AuditVerdict {
        let verdict = match self.live.get(&handle.payload) {
            None => AuditVerdict::NotLive,
            Some(&(generation, _)) if generation == handle.generation => AuditVerdict::Live,
            Some(&(generation, _)) => AuditVerdict::StaleGeneration {
                current: generation,
            },
        };
        if verdict != AuditVerdict::Live {
            self.violations.push((verdict, handle.payload));
        }
        verdict
    }

    /// Release only the allocation identified by `handle`.
    ///
    /// Unlike the guest allocator's raw-pointer free, this boundary rejects a stale handle after
    /// the address has been reused and leaves the newer allocation live.
    pub fn release(&mut self, handle: PoolHandle) -> AuditVerdict {
        match self.live.get(&handle.payload) {
            None => {
                self.violations
                    .push((AuditVerdict::NotLive, handle.payload));
                AuditVerdict::NotLive
            }
            Some(&(generation, _)) if generation != handle.generation => {
                let verdict = AuditVerdict::StaleGeneration {
                    current: generation,
                };
                self.violations.push((verdict, handle.payload));
                verdict
            }
            Some(_) => {
                self.live.remove(&handle.payload);
                AuditVerdict::Live
            }
        }
    }

    pub fn live_count(&self) -> usize {
        self.live.len()
    }

    /// Violations in the order they were observed, with the payload involved.
    pub fn violations(&self) -> &[(AuditVerdict, u32)] {
        &self.violations
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_alloc_free_pairs_are_clean() {
        let mut audit = PoolAudit::new();
        audit.record_alloc(0x1000, 0x40).unwrap();
        assert_eq!(audit.record_free(0x1000), AuditVerdict::Live);
        assert_eq!(audit.record_free(0), AuditVerdict::Live);
        assert!(audit.violations().is_empty());
        assert_eq!(audit.live_count(), 0);
    }

    #[test]
    fn double_and_foreign_free_are_reported() {
        let mut audit = PoolAudit::new();
        audit.record_alloc(0x1000, 0x40).unwrap();
        audit.record_free(0x1000);
        assert_eq!(audit.record_free(0x1000), AuditVerdict::NotLive);
        assert_eq!(audit.record_free(0x2000), AuditVerdict::NotLive);
        assert_eq!(audit.violations().len(), 2);
    }

    #[test]
    fn stale_handle_after_address_reuse_is_detected() {
        let mut audit = PoolAudit::new();
        let old = audit.record_alloc(0x1000, 0x40).unwrap();
        audit.record_free(0x1000);
        assert_eq!(audit.check(old), AuditVerdict::NotLive);
        let new = audit.record_alloc(0x1000, 0x80).unwrap();
        assert_eq!(
            audit.check(old),
            AuditVerdict::StaleGeneration {
                current: new.generation
            }
        );
        assert_eq!(audit.check(new), AuditVerdict::Live);
    }

    #[test]
    fn stale_release_after_address_reuse_preserves_new_allocation() {
        let mut audit = PoolAudit::new();
        let old = audit.record_alloc(0x1000, 0x40).unwrap();
        assert_eq!(audit.release(old), AuditVerdict::Live);
        let new = audit.record_alloc(0x1000, 0x80).unwrap();

        assert_eq!(
            audit.release(old),
            AuditVerdict::StaleGeneration {
                current: new.generation
            }
        );
        assert_eq!(audit.live_count(), 1);
        assert_eq!(audit.check(new), AuditVerdict::Live);
    }

    #[test]
    fn failed_allocation_is_not_tracked() {
        let mut audit = PoolAudit::new();
        assert!(audit.record_alloc(0, 0x40).is_none());
        assert_eq!(audit.live_count(), 0);
    }
}
