// SPDX-License-Identifier: GPL-3.0-or-later
//! Typed game-side allocation boundary (T2392, REPLACE contract in `docs/t1673-rea-memory.md`).
//!
//! This is an opt-in façade over the existing T2048 allocator. It does not change the raw
//! `GameAllocator` / `GuestHeap` behavior or allocator state. The typed layout rejects invalid
//! alignment before reaching the compatibility service; allocation refusal is `Ok(None)`, while
//! guest-memory faults and pool walk limits remain errors. A zero address is never a live
//! allocation in this API.
//!
//! Handles describe the originating heap and requested layout, but are not generation-checked
//! leases. Callers must not retain them across pool reset, reclaim, owner retirement, or arena
//! rewind. The opt-in `owner_lifecycle` feature adds generation-checked leases and the native
//! callback hook; owners capture an `OwnedAllocationPermit` before allocation and consume it when
//! publishing, so synchronous pool callbacks can invalidate identities without borrowing the
//! registry across the allocator call. Concrete game-client state and Rule-24 adoption remain
//! open (T1673).
#[cfg(feature = "native-owner-lifecycle")]
use crate::owner_lifecycle::{NATIVE_OWNER_CALLBACK, NativeOwnerToken};
use crate::{
    MemoryFault,
    heap::{GuestHeap, GuestImageHeap},
    pool::PoolError,
    service_slice::GameAllocator,
    w4m_arena::globals,
};
use core::num::NonZeroU32;
use tsfp_types::GuestAddr;

/// Largest arena alignment exposed by this contract, covering the current 64-byte-aligned uses.
pub const MAX_ARENA_ALIGNMENT: u32 = 64;

/// The game-side storage classes exposed by the current allocator service.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeapKind {
    /// Descending tagged arena. This is the only arena class with a release operation.
    TaggedArena,
    /// Ascending arena regions with shared or separate capacity accounting.
    RegionB,
    RegionA,
    /// Original kind 4, an unchecked ascending cursor.
    UncheckedArena,
    /// Kind-1 pool blocks, freed through the coalescing pool service.
    ReclaimablePool,
    /// Kind-2 pool blocks. The current service does not expose a matching free operation.
    LockedPool,
    /// 16-byte rounded bump scratch storage; individual release is unsupported.
    Scratch,
}

impl HeapKind {
    fn arena_kind(self) -> Option<u32> {
        match self {
            Self::TaggedArena => Some(crate::heap::kind::TAGGED_TOP),
            Self::RegionB => Some(crate::heap::kind::REGION_B),
            Self::RegionA => Some(crate::heap::kind::REGION_A),
            Self::UncheckedArena => Some(crate::heap::kind::UNCHECKED),
            _ => None,
        }
    }
}

/// A nonzero byte size and power-of-two alignment request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    size: NonZeroU32,
    alignment: NonZeroU32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutError {
    ZeroSize,
    ZeroAlignment,
    AlignmentNotPowerOfTwo,
}

impl Layout {
    pub fn new(size: u32, alignment: u32) -> Result<Self, LayoutError> {
        let size = NonZeroU32::new(size).ok_or(LayoutError::ZeroSize)?;
        let alignment = NonZeroU32::new(alignment).ok_or(LayoutError::ZeroAlignment)?;
        if !alignment.get().is_power_of_two() {
            return Err(LayoutError::AlignmentNotPowerOfTwo);
        }
        Ok(Self { size, alignment })
    }

    pub const fn size(self) -> u32 {
        self.size.get()
    }
    pub const fn alignment(self) -> u32 {
        self.alignment.get()
    }
}

/// A typed result from one allocator call. This is descriptive, not an ownership lease.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Allocation {
    address: GuestAddr,
    heap: HeapKind,
    layout: Layout,
}

impl Allocation {
    pub(crate) const fn from_parts(address: GuestAddr, heap: HeapKind, layout: Layout) -> Self {
        Self {
            address,
            heap,
            layout,
        }
    }

    pub const fn address(self) -> GuestAddr {
        self.address
    }
    pub const fn heap(self) -> HeapKind {
        self.heap
    }
    pub const fn layout(self) -> Layout {
        self.layout
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum HeapError {
    Memory(MemoryFault),
    Pool(PoolError),
    UnsupportedAlignment {
        heap: HeapKind,
        alignment: u32,
    },
    MisalignedCursor {
        heap: HeapKind,
        cursor: u32,
        alignment: u32,
    },
    UnsupportedFree(HeapKind),
    ReallocationUnsupported,
    OwnerCallbackUnsupported,
}

impl From<MemoryFault> for HeapError {
    fn from(value: MemoryFault) -> Self {
        Self::Memory(value)
    }
}
impl From<PoolError> for HeapError {
    fn from(value: PoolError) -> Self {
        Self::Pool(value)
    }
}

/// Boundary for native game-side clients. Implementations report allocation refusal as `None`.
/// `reallocate` must return `ReallocationUnsupported` before mutation until a heap-specific move
/// strategy can preserve the old allocation on failure and respect its owner lifetime.
pub trait GameHeapContract {
    fn allocate(&mut self, heap: HeapKind, layout: Layout)
    -> Result<Option<Allocation>, HeapError>;
    fn free(&mut self, allocation: Allocation) -> Result<(), HeapError>;

    /// Allocate a reclaimable buffer with the native owner's opaque token in its pool header.
    /// Prepare an `OwnedAllocationPermit` before this call and publish with that permit after it
    /// returns. The hook set must include `OwnerLifecycleHooks` so reclaim invalidates the lease
    /// before the pool frees or reuses the block.
    #[cfg(feature = "native-owner-lifecycle")]
    fn allocate_owned(
        &mut self,
        _owner: NativeOwnerToken,
        _layout: Layout,
    ) -> Result<Option<Allocation>, HeapError> {
        Err(HeapError::OwnerCallbackUnsupported)
    }

    fn reallocate(
        &mut self,
        _allocation: Allocation,
        _layout: Layout,
    ) -> Result<Allocation, HeapError> {
        Err(HeapError::ReallocationUnsupported)
    }
}

impl<M: crate::GuestMemory, H: crate::heap::HeapHooks> GameHeapContract
    for GuestImageHeap<'_, M, H>
{
    fn allocate(
        &mut self,
        heap: HeapKind,
        layout: Layout,
    ) -> Result<Option<Allocation>, HeapError> {
        let align = layout.alignment();
        let address = if let Some(kind) = heap.arena_kind() {
            if align > MAX_ARENA_ALIGNMENT {
                return Err(HeapError::UnsupportedAlignment {
                    heap,
                    alignment: align,
                });
            }
            GameAllocator::allocate_aligned(self, align, layout.size(), kind)?
        } else {
            match heap {
                HeapKind::ReclaimablePool | HeapKind::LockedPool => {
                    if align > 32 {
                        return Err(HeapError::UnsupportedAlignment {
                            heap,
                            alignment: align,
                        });
                    }
                    let kind = if heap == HeapKind::ReclaimablePool {
                        crate::heap::pool_kind::RECLAIMABLE
                    } else {
                        crate::heap::pool_kind::LOCKED
                    };
                    // The typed interface has no raw guest callback/owner fields. Clients publish
                    // successful allocations through the opt-in native owner registry.
                    GuestHeap::pool_allocate(self, layout.size(), kind, 0, 0)?
                }
                HeapKind::Scratch => {
                    if align > 16 {
                        return Err(HeapError::UnsupportedAlignment {
                            heap,
                            alignment: align,
                        });
                    }
                    let cursor = self.memory.read_u32(GuestAddr(globals::BUMP_CURSOR))?;
                    if cursor % align != 0 {
                        return Err(HeapError::MisalignedCursor {
                            heap,
                            cursor,
                            alignment: align,
                        });
                    }
                    GameAllocator::scratch(self, layout.size())?
                }
                _ => unreachable!("arena heaps handled above"),
            }
        };
        if address == 0 {
            Ok(None)
        } else {
            Ok(Some(Allocation::from_parts(
                GuestAddr(address),
                heap,
                layout,
            )))
        }
    }

    fn free(&mut self, allocation: Allocation) -> Result<(), HeapError> {
        match allocation.heap {
            HeapKind::TaggedArena => {
                GameAllocator::release(self, allocation.address.0).map_err(Into::into)
            }
            HeapKind::ReclaimablePool => {
                GuestHeap::pool_free(self, allocation.address.0).map_err(Into::into)
            }
            heap => Err(HeapError::UnsupportedFree(heap)),
        }
    }

    #[cfg(feature = "native-owner-lifecycle")]
    fn allocate_owned(
        &mut self,
        owner: NativeOwnerToken,
        layout: Layout,
    ) -> Result<Option<Allocation>, HeapError> {
        if layout.alignment() > 32 {
            return Err(HeapError::UnsupportedAlignment {
                heap: HeapKind::ReclaimablePool,
                alignment: layout.alignment(),
            });
        }
        let address = GuestHeap::pool_allocate(
            self,
            layout.size(),
            crate::heap::pool_kind::RECLAIMABLE,
            owner.get(),
            NATIVE_OWNER_CALLBACK,
        )?;
        if address == 0 {
            Ok(None)
        } else {
            Ok(Some(Allocation::from_parts(
                GuestAddr(address),
                HeapKind::ReclaimablePool,
                layout,
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        GuestMemory, heap::OwnedImage, pool::PoolHooks, service_slice::MemoryServices,
        w4m_arena::ArenaHooks,
    };

    #[derive(Default)]
    struct Hooks {
        oom: Vec<u32>,
    }
    impl ArenaHooks for Hooks {
        fn out_of_memory(&mut self, argument: u32) {
            self.oom.push(argument);
        }
    }
    impl PoolHooks for Hooks {
        fn reclaim(&mut self, _: u32, _: u32) {}
    }

    const BASE: u32 = 0x0100_0000;
    const SIZE: u32 = 0x0020_0000;

    fn boot() -> OwnedImage {
        let mut image = OwnedImage::new(0x0040_0000, 0x0400_0000);
        MemoryServices { memory: &mut image }
            .initialize_arena(BASE, SIZE)
            .unwrap();
        image.write_u32(GuestAddr(0x004C_81B8), 0x4000).unwrap();
        let mut hooks = Hooks::default();
        crate::pool::Pool {
            memory: &mut image,
            hooks: &mut hooks,
        }
        .allocate_and_reset()
        .unwrap();
        image
    }

    #[test]
    fn layout_rejects_invalid_size_and_alignment() {
        assert_eq!(Layout::new(0, 16), Err(LayoutError::ZeroSize));
        assert_eq!(Layout::new(1, 0), Err(LayoutError::ZeroAlignment));
        assert_eq!(Layout::new(1, 3), Err(LayoutError::AlignmentNotPowerOfTwo));
        assert_eq!(Layout::new(7, 16).unwrap().size(), 7);
    }

    #[test]
    fn arena_rejects_unvalidated_large_alignment_before_mutation() {
        let mut image = boot();
        let before = image.read_u32(GuestAddr(0x006B_8388)).unwrap();
        let mut hooks = Hooks::default();
        let mut heap = GuestImageHeap {
            memory: &mut image,
            hooks: &mut hooks,
        };
        assert_eq!(
            GameHeapContract::allocate(
                &mut heap,
                HeapKind::TaggedArena,
                Layout::new(16, 128).unwrap()
            ),
            Err(HeapError::UnsupportedAlignment {
                heap: HeapKind::TaggedArena,
                alignment: 128
            })
        );
        assert_eq!(
            heap.memory.read_u32(GuestAddr(0x006B_8388)).unwrap(),
            before
        );
    }

    #[test]
    fn tagged_arena_observes_typed_alignment_and_lifo_free() {
        let mut image = boot();
        let mut hooks = Hooks::default();
        let mut heap = GuestImageHeap {
            memory: &mut image,
            hooks: &mut hooks,
        };
        let layout = Layout::new(37, 16).unwrap();
        let first = GameHeapContract::allocate(&mut heap, HeapKind::TaggedArena, layout)
            .unwrap()
            .unwrap();
        let second = GameHeapContract::allocate(&mut heap, HeapKind::TaggedArena, layout)
            .unwrap()
            .unwrap();
        assert_eq!(first.address().0 % layout.alignment(), 0);
        assert_eq!(second.address().0 % layout.alignment(), 0);
        assert_eq!(first.heap(), HeapKind::TaggedArena);
        GameHeapContract::free(&mut heap, second).unwrap();
        GameHeapContract::free(&mut heap, first).unwrap();
    }

    #[test]
    fn reclaimable_pool_has_fixed_alignment_and_explicit_refusal() {
        let mut image = boot();
        let mut hooks = Hooks::default();
        let mut heap = GuestImageHeap {
            memory: &mut image,
            hooks: &mut hooks,
        };
        let small = Layout::new(100, 32).unwrap();
        let allocation = GameHeapContract::allocate(&mut heap, HeapKind::ReclaimablePool, small)
            .unwrap()
            .unwrap();
        assert_eq!(allocation.address().0 % 32, 0);
        assert_eq!(
            GameHeapContract::allocate(
                &mut heap,
                HeapKind::ReclaimablePool,
                Layout::new(100, 64).unwrap()
            ),
            Err(HeapError::UnsupportedAlignment {
                heap: HeapKind::ReclaimablePool,
                alignment: 64
            })
        );
        assert_eq!(
            GameHeapContract::allocate(
                &mut heap,
                HeapKind::ReclaimablePool,
                Layout::new(0x10_0000, 32).unwrap()
            )
            .unwrap(),
            None
        );
        GameHeapContract::free(&mut heap, allocation).unwrap();
    }

    #[test]
    fn unsupported_reallocation_does_not_touch_the_live_buffer() {
        let mut image = boot();
        let mut hooks = Hooks::default();
        let mut heap = GuestImageHeap {
            memory: &mut image,
            hooks: &mut hooks,
        };
        let old_layout = Layout::new(8, 8).unwrap();
        let allocation =
            GameHeapContract::allocate(&mut heap, HeapKind::ReclaimablePool, old_layout)
                .unwrap()
                .unwrap();
        heap.memory
            .write(allocation.address(), b"REA-test")
            .unwrap();
        assert_eq!(
            GameHeapContract::reallocate(&mut heap, allocation, Layout::new(64, 32).unwrap()),
            Err(HeapError::ReallocationUnsupported)
        );
        let mut bytes = [0; 8];
        heap.memory.read(allocation.address(), &mut bytes).unwrap();
        assert_eq!(&bytes, b"REA-test");
        GameHeapContract::free(&mut heap, allocation).unwrap();
    }

    #[test]
    fn unsupported_free_fails_before_touching_nonreleasable_heap() {
        let mut image = boot();
        let mut hooks = Hooks::default();
        let mut heap = GuestImageHeap {
            memory: &mut image,
            hooks: &mut hooks,
        };
        let allocation =
            GameHeapContract::allocate(&mut heap, HeapKind::RegionA, Layout::new(32, 16).unwrap())
                .unwrap()
                .unwrap();
        assert_eq!(
            GameHeapContract::free(&mut heap, allocation),
            Err(HeapError::UnsupportedFree(HeapKind::RegionA))
        );
    }

    #[test]
    fn arena_oom_hook_is_report_only_and_allocation_continues() {
        let mut image = boot();
        let mut hooks = Hooks::default();
        let mut heap = GuestImageHeap {
            memory: &mut image,
            hooks: &mut hooks,
        };
        let allocation =
            GameHeapContract::allocate(&mut heap, HeapKind::RegionB, Layout::new(SIZE, 1).unwrap())
                .unwrap();
        assert!(allocation.is_some());
        assert_eq!(heap.hooks.oom, [u32::MAX, 1]);
    }

    #[test]
    fn guest_memory_fault_is_not_converted_to_allocation_refusal() {
        let mut image = OwnedImage::new(0x0040_0000, 0x1000);
        let mut hooks = Hooks::default();
        let mut heap = GuestImageHeap {
            memory: &mut image,
            hooks: &mut hooks,
        };
        let error = GameHeapContract::allocate(
            &mut heap,
            HeapKind::TaggedArena,
            Layout::new(32, 16).unwrap(),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            HeapError::Memory(crate::MemoryFault {
                access: crate::Access::Read,
                ..
            })
        ));
    }

    #[test]
    fn locked_pool_allocation_has_no_individual_free_contract() {
        let mut image = boot();
        let mut hooks = Hooks::default();
        let mut heap = GuestImageHeap {
            memory: &mut image,
            hooks: &mut hooks,
        };
        let allocation = GameHeapContract::allocate(
            &mut heap,
            HeapKind::LockedPool,
            Layout::new(64, 32).unwrap(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            GameHeapContract::free(&mut heap, allocation),
            Err(HeapError::UnsupportedFree(HeapKind::LockedPool))
        );
    }

    #[test]
    fn scratch_alignment_is_checked_against_its_current_cursor() {
        let mut image = boot();
        let mut hooks = Hooks::default();
        let mut heap = GuestImageHeap {
            memory: &mut image,
            hooks: &mut hooks,
        };
        let cursor = BASE + 0x100;
        heap.memory
            .write_u32(GuestAddr(globals::BUMP_CURSOR), cursor)
            .unwrap();
        let allocation =
            GameHeapContract::allocate(&mut heap, HeapKind::Scratch, Layout::new(7, 16).unwrap())
                .unwrap()
                .unwrap();
        assert_eq!(allocation.address().0, cursor);
        assert_eq!(allocation.address().0 % 16, 0);

        let misaligned = cursor + 4;
        heap.memory
            .write_u32(GuestAddr(globals::BUMP_CURSOR), misaligned)
            .unwrap();
        assert_eq!(
            GameHeapContract::allocate(&mut heap, HeapKind::Scratch, Layout::new(7, 16).unwrap()),
            Err(HeapError::MisalignedCursor {
                heap: HeapKind::Scratch,
                cursor: misaligned,
                alignment: 16
            })
        );
        assert_eq!(
            heap.memory
                .read_u32(GuestAddr(globals::BUMP_CURSOR))
                .unwrap(),
            misaligned
        );
    }

    #[cfg(feature = "native-owner-lifecycle")]
    #[test]
    fn owned_pool_allocation_uses_typed_token_and_native_callback_marker() {
        let mut image = boot();
        let mut registry = crate::owner_lifecycle::OwnerRegistry::new();
        let owner = registry.register_owner().unwrap();
        let token = registry.owner_token(owner).unwrap();
        let mut base_hooks = Hooks::default();
        let layout = Layout::new(0x100, 0x10).unwrap();
        let (id, allocation) = crate::owner_lifecycle::allocate_owned_with_registry(
            &mut image,
            &mut base_hooks,
            &mut registry,
            owner,
            layout,
            0x10,
            |_: &mut crate::owner_lifecycle::OwnerRegistry,
             _: &crate::owner_lifecycle::ReclaimEvent| {},
        )
        .unwrap()
        .unwrap();

        let header = allocation.address().0 - crate::pool::HEADER;
        assert_eq!(
            image.read_u32(GuestAddr(header + 0x0C)).unwrap(),
            token.get()
        );
        assert_eq!(
            image.read_u32(GuestAddr(header + 0x10)).unwrap(),
            crate::owner_lifecycle::NATIVE_OWNER_CALLBACK
        );
        assert_eq!(registry.live_lease(id).unwrap().owner_token(), Some(token));
    }
}
