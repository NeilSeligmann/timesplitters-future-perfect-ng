// SPDX-License-Identifier: GPL-3.0-or-later
//! T2048 `GuestHeap`: the single allocator boundary for the game side (docs/t-memory-replace.md).
//!
//! Callers that today reach 0x3DFA0 / 0x3E0B0 / 0x3E970 / 0x3E1E0 / 0x3ED20 / 0x3EAD0 depend on
//! this trait instead of on guest globals. [`GuestImageHeap`] implements it over any
//! [`GuestMemory`]: a mapping of the real guest image (lifted-code bridge, addresses identical to
//! the original) or an owned [`OwnedImage`] buffer (Rust-owned data, tests, S3). Both keep the
//! allocator state in the guest globals, so address stability holds by construction.
use crate::{
    GuestMemory, MemoryFault,
    pool::{Pool, PoolError, PoolHooks},
    service_slice::GameAllocator,
    w4m_arena::{Arena, ArenaHooks},
};
use tsfp_types::GuestAddr;

/// Allocation kinds of the original 0x3DFA0 dispatch (note: kind 3 is excluded and returns 0).
pub mod kind {
    pub const TAGGED_TOP: u32 = 0;
    pub const REGION_B: u32 = 1;
    pub const REGION_A: u32 = 2;
    pub const UNCHECKED: u32 = 4;
}

/// The pool kinds of 0x3ED20.
pub mod pool_kind {
    pub const RECLAIMABLE: u32 = 1;
    pub const LOCKED: u32 = 2;
}

/// Everything the game side allocates through.
pub trait GuestHeap: GameAllocator {
    /// 0x3ED20(size, kind, argument, callback). 0 means the request was refused.
    fn pool_allocate(
        &mut self,
        size: u32,
        kind: u32,
        argument: u32,
        callback: u32,
    ) -> Result<u32, PoolError>;
    /// 0x3EAD0: null tolerant.
    fn pool_free(&mut self, pointer: u32) -> Result<(), PoolError>;
}

/// Hooks for the host side effects (out-of-memory reports, reclaim callbacks).
pub trait HeapHooks: ArenaHooks + PoolHooks {}
impl<T: ArenaHooks + PoolHooks> HeapHooks for T {}

pub struct GuestImageHeap<'a, M: GuestMemory, H: HeapHooks> {
    pub memory: &'a mut M,
    pub hooks: &'a mut H,
}

impl<M: GuestMemory, H: HeapHooks> GuestImageHeap<'_, M, H> {
    fn arena(&mut self) -> Arena<'_, M, H> {
        Arena {
            memory: &mut *self.memory,
            hooks: &mut *self.hooks,
        }
    }
    fn pool(&mut self) -> Pool<'_, M, H> {
        Pool {
            memory: &mut *self.memory,
            hooks: &mut *self.hooks,
        }
    }
}

type Fault<T> = Result<T, MemoryFault>;

impl<M: GuestMemory, H: HeapHooks> GameAllocator for GuestImageHeap<'_, M, H> {
    fn allocate(&mut self, kind: u32, size: u32) -> Fault<u32> {
        self.arena().allocate(kind, size)
    }
    fn allocate_aligned(&mut self, alignment: u32, size: u32, kind: u32) -> Fault<u32> {
        self.arena().allocate_aligned(alignment, size, kind)
    }
    fn release(&mut self, pointer: u32) -> Fault<()> {
        self.arena().release(pointer)
    }
    fn scratch(&mut self, size: u32) -> Fault<u32> {
        self.arena().scratch(size)
    }
}

impl<M: GuestMemory, H: HeapHooks> GuestHeap for GuestImageHeap<'_, M, H> {
    fn pool_allocate(
        &mut self,
        size: u32,
        kind: u32,
        argument: u32,
        callback: u32,
    ) -> Result<u32, PoolError> {
        self.pool().allocate_by_kind(size, kind, argument, callback)
    }
    fn pool_free(&mut self, pointer: u32) -> Result<(), PoolError> {
        self.pool().free(pointer)
    }
}

/// A Rust-owned guest image: a zeroed byte buffer at a guest base. Lets Rust-owned code (and the
/// tests) run the same allocators with no C and no lifted code behind it.
pub struct OwnedImage {
    pub base: u32,
    pub bytes: Vec<u8>,
}
impl OwnedImage {
    pub fn new(base: u32, length: usize) -> Self {
        Self {
            base,
            bytes: vec![0; length],
        }
    }
    fn span(
        &self,
        address: GuestAddr,
        length: usize,
        access: crate::Access,
    ) -> Fault<core::ops::Range<usize>> {
        let fault = MemoryFault {
            address,
            length,
            access,
        };
        let start = address.0.checked_sub(self.base).ok_or(fault)? as usize;
        let end = start.checked_add(length).ok_or(fault)?;
        if end > self.bytes.len() {
            Err(fault)
        } else {
            Ok(start..end)
        }
    }
}
impl GuestMemory for OwnedImage {
    fn read(&mut self, address: GuestAddr, out: &mut [u8]) -> Fault<()> {
        let range = self.span(address, out.len(), crate::Access::Read)?;
        out.copy_from_slice(&self.bytes[range]);
        Ok(())
    }
    fn write(&mut self, address: GuestAddr, bytes: &[u8]) -> Fault<()> {
        let range = self.span(address, bytes.len(), crate::Access::Write)?;
        self.bytes[range].copy_from_slice(bytes);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service_slice::MemoryServices;

    #[derive(Default)]
    struct Hooks {
        oom: Vec<u32>,
        reclaimed: Vec<(u32, u32)>,
    }
    impl ArenaHooks for Hooks {
        fn out_of_memory(&mut self, argument: u32) {
            self.oom.push(argument);
        }
    }
    impl PoolHooks for Hooks {
        fn reclaim(&mut self, callback: u32, argument: u32) {
            self.reclaimed.push((callback, argument));
        }
    }

    const BASE: u32 = 0x0100_0000;
    const SIZE: u32 = 0x0020_0000;

    /// Boot-like setup: 0x3DF00 arena init, then the pool buffer through 0x3EA00.
    fn boot() -> OwnedImage {
        let mut image = OwnedImage::new(0x0040_0000, 0x0400_0000);
        // OwnedImage covers 0x400000..0x4400000: globals sit below 0x800000, the arena above.
        MemoryServices { memory: &mut image }
            .initialize_arena(BASE, SIZE)
            .unwrap();
        image.write_u32(GuestAddr(0x004C_81B8), 0x4000).unwrap();
        let mut hooks = Hooks::default();
        Pool {
            memory: &mut image,
            hooks: &mut hooks,
        }
        .allocate_and_reset()
        .unwrap();
        image
    }

    #[test]
    fn pool_kind1_alloc_free_roundtrip_keeps_accounting() {
        let mut image = boot();
        let mut hooks = Hooks::default();
        let free_before = image.read_u32(GuestAddr(0x006B_83C0)).unwrap();
        let mut heap = GuestImageHeap {
            memory: &mut image,
            hooks: &mut hooks,
        };
        let a = heap
            .pool_allocate(100, pool_kind::RECLAIMABLE, 0, 0)
            .unwrap();
        let b = heap
            .pool_allocate(300, pool_kind::RECLAIMABLE, 0, 0)
            .unwrap();
        assert_ne!(a, 0);
        assert_eq!(a % 32, 0);
        assert!(b > a);
        heap.pool_free(a).unwrap();
        heap.pool_free(0).unwrap();
        heap.pool_free(b).unwrap();
        assert_eq!(image.read_u32(GuestAddr(0x006B_83C0)).unwrap(), free_before);
        assert_eq!(image.read_u32(GuestAddr(0x006B_83BC)).unwrap(), 1);
    }

    #[test]
    fn nullable_pool_free_zero_preserves_guest_image() {
        let mut image = boot();
        let before = image.bytes.clone();
        let mut hooks = Hooks::default();
        GuestImageHeap {
            memory: &mut image,
            hooks: &mut hooks,
        }
        .pool_free(0)
        .unwrap();
        assert_eq!(image.bytes, before);
    }

    #[test]
    fn pool_reclaims_with_callback_when_full() {
        let mut image = boot();
        let mut hooks = Hooks::default();
        let mut heap = GuestImageHeap {
            memory: &mut image,
            hooks: &mut hooks,
        };
        let mut live = vec![];
        loop {
            let p = heap
                .pool_allocate(0x700, pool_kind::RECLAIMABLE, 0x1234, 0xCAFE)
                .unwrap();
            if p == 0 || !heap.hooks.reclaimed.is_empty() {
                break;
            }
            live.push(p);
            assert!(live.len() < 100);
        }
        assert!(!live.is_empty());
        assert_eq!(heap.hooks.reclaimed.first(), Some(&(0xCAFE, 0x1234)));
    }

    #[test]
    fn arena_kinds_through_the_trait() {
        let mut image = boot();
        let mut hooks = Hooks::default();
        let mut heap = GuestImageHeap {
            memory: &mut image,
            hooks: &mut hooks,
        };
        let a = heap.allocate(kind::TAGGED_TOP, 0x40).unwrap();
        let b = heap.allocate(kind::TAGGED_TOP, 0x40).unwrap();
        assert!(b < a);
        heap.release(b).unwrap();
        heap.release(a).unwrap();
        assert!(heap.hooks.oom.is_empty());
    }
}
