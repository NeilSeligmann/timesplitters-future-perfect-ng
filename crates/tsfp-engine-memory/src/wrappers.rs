// SPDX-License-Identifier: GPL-3.0-or-later
//! T2048 slice W: the `game_alloc_*` wrappers, ported over the Rust allocators.
//!
//! Each wrapper in the original is "allocate one or more blocks (ledger 0x3E440 or arena 0x3DFA0),
//! publish the pointer in its own guest global, zero it". They are the constructors of the S3 owned
//! tables. While the guest image exists the pointer globals stay at their original addresses, so
//! the lifted code that reads them keeps working; once S3 owns a table, the matching function here
//! is the constructor to replace (the published pointer becomes a typed handle). The
//! allocator state is only reached through [`Ledgers`] and [`Arena`], so the Rust ledgers and
//! arena are the only allocator in the loop.
//!
//! Evidence class: Tier 1 differential against the lifted original (crates/tsfp-c-diff,
//! `w4_ledger.rs`, wrapper tests), fabricated allocator state. Nothing here repairs the original:
//! a disabled ledger (flag 0x4C81B4 == 0) makes 0x3E440 return 0 and the wrapper still zeroes
//! from address 0, exactly as the original would.
use crate::{
    GuestMemory, MemoryFault,
    contract::{GameHeapContract, HeapError, HeapKind, Layout},
    heap::GuestImageHeap,
    ledger::Ledgers,
    pool::{Pool, PoolError, PoolHooks},
    service_slice::GameAllocator,
    w4m_arena::{Arena, ArenaHooks, TaggedReleaseRegisters},
};
use tsfp_types::GuestAddr;

type Fault<T> = Result<T, MemoryFault>;

/// Pool hooks are required by `GuestImageHeap`, but this wrapper only allocates from Region B.
/// Keep its arena report callback while making an accidental pool-reclaim call inert and local.
struct ArenaContractHooks<'a, H: ArenaHooks>(&'a mut H);

impl<H: ArenaHooks> ArenaHooks for ArenaContractHooks<'_, H> {
    fn out_of_memory(&mut self, argument: u32) {
        self.0.out_of_memory(argument);
    }
}

impl<H: ArenaHooks> PoolHooks for ArenaContractHooks<'_, H> {
    fn reclaim(&mut self, _callback: u32, _argument: u32) {}
}

const AUDIO_DESCRIPTOR_TABLE: u32 = 0x4CA9B4;
const AUDIO_DESCRIPTOR_STRIDE: u32 = 0x14;
const AUDIO_DESCRIPTOR_READY_OFFSET: u32 = 0x0C;

/// Random source used by the original point-table constructor. The host supplies the guest
/// `0x153B40` stream so allocator tests and the live adapter share the same call order.
pub trait PointPoolRandom {
    fn next_point_random(&mut self) -> u32;
}

/// Native body of the game-pool reclaim callback at `0x210FF0`.
///
/// The lifted callback clears the random-point count before publishing a null owner pointer.
/// Keep the two guest writes ordered and available to the host adapter without calling back into
/// lifted code. The guest image remains the address-stable backing store until S3 migrates this
/// owner into Rust state.
pub fn clear_random_point_pool_owner(memory: &mut impl GuestMemory) -> Fault<()> {
    memory.write_u32(GuestAddr(0x75C0A8), 0)?;
    memory.write_u32(GuestAddr(0x75C0CC), 0)
}

/// Typed guest-image view for a particle emitter that owns a reclaimable buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParticleBufferOwner(GuestAddr);

impl ParticleBufferOwner {
    pub const fn from_guest_address(address: u32) -> Self {
        Self(GuestAddr(address))
    }

    pub const fn guest_address(self) -> u32 {
        self.0.0
    }
}

/// Release and invalidate the reclaimable particle buffer owned by one guest emitter.
///
/// The original `0x10C0E0` first frees a non-null payload through nullable pool free, then checks
/// flag bit 0 at owner `+4` and conditionally publishes state `1` at `+0x50`. It finally clears
/// the four buffer words at `+0x40..+0x50`. The guest image remains the stable-address backing
/// store while owner records have not yet migrated to Rust-owned state.
pub fn release_particle_buffer<M, H>(
    memory: &mut M,
    hooks: &mut H,
    owner: ParticleBufferOwner,
) -> Result<(), PoolError>
where
    M: GuestMemory,
    H: PoolHooks + ArenaHooks,
{
    let owner_address = owner.guest_address();
    if owner_address == 0 {
        return Ok(());
    }

    let payload = memory.read_u32(GuestAddr(owner_address.wrapping_add(0x40)))?;
    if payload != 0 {
        Pool {
            memory: &mut *memory,
            hooks,
        }
        .free(payload)?;

        // The C ABI adapter only exposes guest-word access. The flag is byte-sized in the
        // retained contract, and its address is word-aligned, so read the containing word and
        // test the low bit without requiring a byte-read callback.
        let flags = memory.read_u32(GuestAddr(owner_address.wrapping_add(4)))?;
        if flags & 1 != 0 {
            memory.write_u32(GuestAddr(owner_address.wrapping_add(0x50)), 1)?;
        }
    }

    for offset in [0x40, 0x44, 0x48, 0x4C] {
        memory.write_u32(GuestAddr(owner_address.wrapping_add(offset)), 0)?;
    }
    Ok(())
}

/// Result published by the particle buffer constructor at `0x10C190`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParticleBufferAllocation {
    pub entry_count: u32,
    pub payload: u32,
}

/// Count and byte request computed by the retained `0x10C190` caller before pool allocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParticleBufferRequest {
    pub entry_count: u32,
    pub byte_len: u32,
}

/// Apply the original signed cap and descriptor stride to the count returned by `0x10C120`.
pub const fn particle_buffer_request(
    entry_count: i32,
    descriptor_kind: u32,
) -> ParticleBufferRequest {
    let entry_count = if entry_count >= 500 {
        500
    } else {
        entry_count as u32
    };
    let stride = if matches!(descriptor_kind, 5 | 6) {
        0x58
    } else {
        0x34
    };
    ParticleBufferRequest {
        entry_count,
        byte_len: entry_count.wrapping_mul(stride),
    }
}

/// Publish the retained count-before-pointer pair after the allocator returns.
pub fn publish_particle_buffer<M: GuestMemory>(
    memory: &mut M,
    owner: ParticleBufferOwner,
    request: ParticleBufferRequest,
    payload: u32,
) -> Fault<()> {
    let owner_address = owner.guest_address();
    if owner_address == 0 {
        return Ok(());
    }
    memory.write_u32(
        GuestAddr(owner_address.wrapping_add(0x44)),
        request.entry_count,
    )?;
    memory.write_u32(GuestAddr(owner_address.wrapping_add(0x40)), payload)
}

/// Allocate a particle emitter's kind-1 pool buffer and publish its count before its payload.
///
/// The host adapter performs the retained `0x10C0E0` release and `0x10C120` count helper calls,
/// then passes their results here. A non-null stored guest callback remains a TEMPORARY-SHIM
/// through `PoolHooks` until owner identities move into Rust-owned state under T2044 S3.
pub fn allocate_particle_buffer<M, H>(
    memory: &mut M,
    hooks: &mut H,
    owner: ParticleBufferOwner,
    entry_count: i32,
    descriptor_kind: u32,
) -> Result<Option<ParticleBufferAllocation>, PoolError>
where
    M: GuestMemory,
    H: PoolHooks + ArenaHooks,
{
    let owner_address = owner.guest_address();
    if owner_address == 0 {
        return Ok(None);
    }
    let request = particle_buffer_request(entry_count, descriptor_kind);
    let payload = Pool {
        memory: &mut *memory,
        hooks,
    }
    .allocate_by_kind(request.byte_len, 1, owner_address, 0x10C0E0)?;

    // The original publishes the requested count first, including a failed zero allocation.
    publish_particle_buffer(memory, owner, request, payload)?;
    Ok(Some(ParticleBufferAllocation {
        entry_count: request.entry_count,
        payload,
    }))
}

/// Wrapper constructors over a guest memory.
pub struct AllocWrappers<'a, M: GuestMemory, H: ArenaHooks> {
    pub memory: &'a mut M,
    pub hooks: &'a mut H,
}

impl<M: GuestMemory, H: ArenaHooks> AllocWrappers<'_, M, H> {
    fn get(&mut self, a: u32) -> Fault<u32> {
        self.memory.read_u32(GuestAddr(a))
    }
    fn put(&mut self, a: u32, v: u32) -> Fault<()> {
        self.memory.write_u32(GuestAddr(a), v)
    }
    /// Reset one descriptor's readiness word in the shared 20-byte table.
    fn reset_table_descriptor_readiness(&mut self, index: u32) -> Fault<()> {
        let table = self.get(AUDIO_DESCRIPTOR_TABLE)?;
        let field = table
            .wrapping_add(index.wrapping_mul(AUDIO_DESCRIPTOR_STRIDE))
            .wrapping_add(AUDIO_DESCRIPTOR_READY_OFFSET);
        self.put(field, u32::MAX)
    }
    /// 0x3E440 through the Rust ledger.
    fn ledger(&mut self, size: u32) -> Fault<u32> {
        Ledgers {
            memory: &mut *self.memory,
        }
        .allocate_primary(size)
    }
    /// 0x3DFA0 through the Rust arena (original order `(size, kind)`).
    fn arena(&mut self, kind: u32, size: u32) -> Fault<u32> {
        Arena {
            memory: &mut *self.memory,
            hooks: &mut *self.hooks,
        }
        .alloc_round16(kind, size)
    }
    /// Enter the shared typed heap contract for this Region-B allocation. Alignment 1 preserves
    /// the original size-rounding-only request; stronger alignment would add cursor padding.
    fn typed_region_b(&mut self, size: u32) -> Result<u32, HeapError> {
        let layout = Layout::new(size, 1).expect("fixed Region-B wrapper size is nonzero");
        let mut hooks = ArenaContractHooks(&mut *self.hooks);
        let mut heap = GuestImageHeap {
            memory: &mut *self.memory,
            hooks: &mut hooks,
        };
        Ok(
            GameHeapContract::allocate(&mut heap, HeapKind::RegionB, layout)?
                .map_or(0, |allocation| allocation.address().0),
        )
    }
    /// 0x12C800: lease the next stable slot from the 17-entry guest table.
    /// The table remains in the guest image until S3 moves its owner to Rust.
    pub fn allocate_slot_745938_max17(&mut self) -> Fault<u32> {
        let count = self.get(0x7490A0)?;
        if count >= 0x11 {
            return Ok(0);
        }
        self.put(0x7490A0, count + 1)?;
        Ok(0x745938 + count * 0x144)
    }
    /// 0x1D1E80: allocate the 0x1590-byte table at 0x20 alignment in region B.
    /// The table's contents belong to its caller; this constructor only publishes its address.
    pub fn alloc_aligned_75b4b4(&mut self) -> Fault<()> {
        let block = Arena {
            memory: &mut *self.memory,
            hooks: &mut *self.hooks,
        }
        .allocate_aligned(0x20, 0x1590, 1)?;
        self.put(0x75B4B4, block)
    }
    /// 0x24A180: allocate the 0x640-byte kind-1 arena block at 0x20 alignment and publish it
    /// before the original tail-called initializer at 0x2498C0 runs.
    pub fn alloc_model_buffer_78b718(&mut self) -> Fault<u32> {
        let block = Arena {
            memory: &mut *self.memory,
            hooks: &mut *self.hooks,
        }
        .allocate_aligned(0x20, 0x640, 1)?;
        self.put(0x78B718, block)?;
        Ok(block)
    }
    /// 0x271910: allocate the first weather table and conditionally allocate the second.
    /// The signed gate byte is read from the guest image; the caller-owned tables stay there.
    pub fn alloc_weather_tables_74c3b0(&mut self) -> Fault<u32> {
        let first = self.arena(1, 0x550)?;
        self.put(0x74C3B0, first)?;
        let gate = ((self.get(0x7DE400)? >> 16) as u8) as i8;
        if gate > 0 {
            let second = self.arena(1, 0x970)?;
            self.put(0x74C3B4, second)?;
            Ok(second)
        } else {
            Ok((first & 0xFFFF_FF00) | (gate as u8 as u32))
        }
    }

    /// 0x119ED0: allocate the weather's 0x900-byte zeroed table and its 0xF0-byte rows,
    /// then reset the two resource ids and one timer field. Its original record is INFERRED
    /// and skips an SSE/MMX region; only the allocation tuples and listed writes are modeled.
    pub fn alloc_weather_tables_with_reset_7ac824(&mut self) -> Fault<()> {
        let fixed = self.arena_table(0x900, 0x7AC824)?;
        self.zero_dwords(fixed, 0x240)?;

        let rows = self.get(0x73D994)?;
        let row_bytes = rows.wrapping_mul(0xF0);
        self.arena_table(row_bytes, 0x7AC828)?;

        self.put(0x4E62E0, u32::MAX)?;
        self.put(0x4E62E4, u32::MAX)?;
        self.put(0x73D9B0, 0)
    }
    /// 0x188900: allocate and publish the 0x194-byte effect-slot table.
    /// The original tail-calls the neighboring 0x188870 initializer after this seam.
    pub fn alloc_effect_slots_7a156c(&mut self) -> Fault<u32> {
        let block = Arena {
            memory: &mut *self.memory,
            hooks: &mut *self.hooks,
        }
        .allocate_aligned(0x20, 0x194, 1)?;
        self.put(0x7A156C, block)?;
        Ok(block)
    }
    /// 0x298B0: allocate the 0x36000-byte, 16-byte-aligned kind-1 workspace and publish it
    /// into the caller-owned audio record at offset +8. The record remains guest-image-backed.
    pub fn alloc_audio_workspace_298b0(&mut self, owner: u32) -> Fault<u32> {
        let block = Arena {
            memory: &mut *self.memory,
            hooks: &mut *self.hooks,
        }
        .allocate_aligned(0x10, 0x36000, 1)?;
        self.put(owner.wrapping_add(8), block)?;
        Ok(block)
    }

    /// 0x2BC150: allocate the fixed 0xD818-byte effect-mesh buffer in arena kind 1 and
    /// publish its stable guest address at 0x78AEF0. The original tail-calls 0x2BBC40;
    /// the host adapter retains that builder and its caller stack frame.
    pub fn alloc_effect_mesh_buffer_78aef0(&mut self) -> Fault<u32> {
        let block = self.arena(1, 0xD818)?;
        self.put(0x78AEF0, block)?;
        Ok(block)
    }

    /// Allocate and publish the byte-sized Story event instance array in Region B.
    /// The lifted caller computes `count * 0x48`; initialization remains at 0x235630.
    pub fn alloc_story_event_instances_78cd90(&mut self, size: u32) -> Fault<u32> {
        let block = self.arena(1, size)?;
        self.put(0x78CD90, block)?;
        Ok(block)
    }
    /// Allocate and publish the active-instance index list used by the Story event pool.
    pub fn alloc_story_event_indices_78cd8c(&mut self, size: u32) -> Fault<u32> {
        let block = self.arena(1, size)?;
        self.put(0x78CD8C, block)?;
        Ok(block)
    }
    /// Allocate and publish the Story event snapshot buffer; the caller performs the original
    /// direction-flag-aware zeroing after allocation.
    pub fn alloc_story_event_snapshot_75cca4(&mut self, size: u32) -> Fault<u32> {
        let block = self.arena(1, size)?;
        self.put(0x75CCA4, block)?;
        Ok(block)
    }
    /// Allocate the two support arrays after the instance array, preserving allocation order.
    /// A fault on the second allocation leaves the first allocation and publication in place.
    pub fn alloc_story_event_support_arrays(
        &mut self,
        index_size: u32,
        snapshot_size: u32,
    ) -> Fault<(u32, u32)> {
        let indices = self.alloc_story_event_indices_78cd8c(index_size)?;
        let snapshot = self.alloc_story_event_snapshot_75cca4(snapshot_size)?;
        Ok((indices, snapshot))
    }
    /// Region-B arena block (0x3DFA0 kind 1), published in `global`; the table contents belong
    /// to the caller. Returns the block.
    fn arena_table(&mut self, size: u32, global: u32) -> Fault<u32> {
        let block = self.arena(1, size)?;
        self.put(global, block)?;
        Ok(block)
    }

    /// 0x6B360: 0xE0-byte table at 0x6F1E18, with its fill count 0x6F1E20 cleared first.
    pub fn alloc_table_6f1e18(&mut self) -> Fault<()> {
        self.put(0x6F1E20, 0)?;
        self.arena_table(0xE0, 0x6F1E18).map(|_| ())
    }

    /// 0xB05B0: zeroed 0x26A0-byte table at 0x732A04, count 0x732A08 cleared.
    pub fn alloc_zeroed_732a04(&mut self) -> Fault<()> {
        let block = self.arena_table(0x26A0, 0x732A04)?;
        self.put(0x732A08, 0)?;
        self.zero_dwords(block, 0x9A8)
    }

    /// 0xC3540: zeroed 0x14A0-byte table at 0x732E70, count 0x732E74 cleared.
    pub fn alloc_zeroed_732e70(&mut self) -> Fault<()> {
        let block = self.arena_table(0x14A0, 0x732E70)?;
        self.put(0x732E74, 0)?;
        self.zero_dwords(block, 0x528)
    }

    /// 0x21AF30: 0x10-byte block at 0x75C0F0.
    pub fn alloc_table_75c0f0(&mut self) -> Fault<()> {
        self.arena_table(0x10, 0x75C0F0).map(|_| ())
    }

    /// 0x2210B0: 0x40-byte block at 0x75C10C.
    pub fn alloc_table_75c10c(&mut self) -> Fault<()> {
        self.arena_table(0x40, 0x75C10C).map(|_| ())
    }

    /// 0x211040: allocate the 0x1770-byte point table through the reclaimable pool, publish it
    /// at 0x75C0CC, then fill one 0x14-byte randomized point when allocation succeeds. The
    /// original returns the resulting count in EAX (0 or 1), not the allocated pointer.
    pub fn alloc_random_point_pool_75c0cc(&mut self) -> Result<u32, PoolError>
    where
        H: PoolHooks + PointPoolRandom,
    {
        let block = Pool {
            memory: &mut *self.memory,
            hooks: &mut *self.hooks,
        }
        .allocate_by_kind(0x1770, 1, 0x75C0CC, 0x210FF0)?;
        self.put(0x75C0CC, block)?;
        let count = u32::from(block != 0);
        self.put(0x75C0A8, count)?;
        if count == 0 {
            return Ok(0);
        }

        let coordinate_center = f32::from_bits(self.get(0x0047_862C)?);
        let unit_scale = f32::from_bits(self.get(0x0047_85D0)?);
        let size_scale = f32::from_bits(self.get(0x0047_5CD4)?);
        let size_offset = f32::from_bits(self.get(0x0047_5D40)?);
        let size_final_scale = f32::from_bits(self.get(0x0047_5DB4)?);

        let x = (self.hooks.next_point_random() % 0x14) as f32 - coordinate_center;
        let y = (self.hooks.next_point_random() % 0x14) as f32 - coordinate_center;
        let z_random = self.hooks.next_point_random();
        // The lifted x87 FILD reads the random dword as a signed integer.
        let z = (((z_random as i32) as f64) * f64::from(unit_scale)) as f32;
        let size_random = self.hooks.next_point_random();
        let size = ((((size_random as i32) as f64) * f64::from(size_scale)
            + f64::from(size_offset))
            * f64::from(size_final_scale)) as f32;

        self.put(block, x.to_bits())?;
        self.put(block.wrapping_add(4), y.to_bits())?;
        self.put(block.wrapping_add(8), z.to_bits())?;
        self.put(block.wrapping_add(0xC), size.to_bits())?;
        self.put(block.wrapping_add(0x10), 1)?;
        Ok(count)
    }

    /// 0x220E20: allocate a 0x10-byte state block at 0x75C108 and clear readiness
    /// for descriptor 0x18 in the 20-byte table rooted at [0x4CA9B4].
    pub fn alloc_state_block_75c108(&mut self) -> Fault<()> {
        let block = self.arena(1, 0x10)?;
        self.put(0x75C108, block)?;
        self.reset_table_descriptor_readiness(0x18)
    }

    /// 0x220E40: initialize the published 0x10-byte state block at [0x75C108].
    /// The original writes zero to both dwords, then overwrites the second with u32::MAX.
    pub fn initialize_state_block_75c108(&mut self) -> Fault<u32> {
        let block = self.get(0x75C108)?;
        self.put(block, 0)?;
        self.put(block.wrapping_add(4), 0)?;
        self.put(block.wrapping_add(4), u32::MAX)?;
        Ok(block)
    }

    /// 0x2259D0: allocate the 16-byte sound-state block and reset the two loop descriptors.
    /// The allocation is published at 0x75C124; T2048 infers this global from the state used by
    /// the 0x225840 sound hook. The descriptor table remains in the guest image.
    pub fn alloc_sound_state_75c124(&mut self) -> Fault<()> {
        let block = self.arena(1, 0x10)?;
        self.put(0x75C124, block)?;
        self.reset_table_descriptor_readiness(0x1B)?;
        self.reset_table_descriptor_readiness(0x19)
    }

    /// 0x2226F0: allocate the timer state and reset its five shared-table descriptors.
    pub fn alloc_timer_state_75c118(&mut self) -> Result<(), HeapError> {
        let block = self.typed_region_b(0x10)?;
        self.put(0x75C118, block)?;
        for index in [0x156F, 0x1570, 0x5F3, 0x5F4, 0x5F5] {
            self.reset_table_descriptor_readiness(index)?;
        }
        Ok(())
    }

    /// 0x2A62C0: 0x7D00-byte block at 0x78B0D0.
    pub fn alloc_table_78b0d0(&mut self) -> Fault<()> {
        self.arena_table(0x7D00, 0x78B0D0).map(|_| ())
    }

    /// 0x2A8300: select the mode's pool count and allocate/publish both kind-1 buffers.
    /// The guest image remains authoritative for the three published globals.
    pub fn alloc_tables_78b08c(&mut self, flag: u32) -> Fault<u32> {
        let (count, size) = if flag != 0 {
            (0x1F4, 0x55F0)
        } else {
            (0x578, 0xF0A0)
        };
        self.put(0x78B08C, count)?;
        let first = self.arena(1, size)?;
        self.put(0x78B098, first)?;
        let second = self.arena(1, size)?;
        self.put(0x78B090, second)?;
        Ok(second)
    }

    /// 0x31AAA0: allocate a tagged 0x14-byte block, copy four source words, publish its guest
    /// pointer at 0x762F7C, then copy the fifth word. The publication-before-final-copy order
    /// preserves the original partial state if the fifth store faults. The caller passes the
    /// source address in its inherited stack slot even though the retained entry is cdecl/0.
    /// Verification: Tier 1 against the lifted entry over 3,000 FABRICATED arena states.
    pub fn alloc_pick_node_copy_762f7c(&mut self, source: u32) -> Fault<u32> {
        let block = self.arena(0, 0x14)?;
        for index in 0..4 {
            let value = self.get(source.wrapping_add(index * 4))?;
            self.put(block.wrapping_add(index * 4), value)?;
        }
        let fifth = self.get(source.wrapping_add(0x10))?;
        self.put(0x762F7C, block)?;
        self.put(block.wrapping_add(0x10), fifth)?;
        Ok(block)
    }

    /// Initialize the guest node-pool record and its intrusive freelist in retained write order.
    /// The descriptor stores buffer, freelist head, capacity and free count at `pool+0/4/8/12`;
    /// each node's first dword points to the previously visited node.
    fn init_node_pool_freelist(
        &mut self,
        pool: u32,
        nodes: u32,
        stride: u32,
        count: u32,
    ) -> Fault<()> {
        crate::fixed_pool::initialize_freelist(self.memory, pool, stride, nodes, count).map(|_| ())
    }

    /// 0x3796B0: allocate 0x960 bytes from kind 1 and initialize the 0x772B50 node pool with
    /// 0xF entries of stride 0xA0. The allocation and node storage remain in the guest image.
    /// Verification: Tier 1 against the lifted constructor over 3,000 FABRICATED arena states.
    pub fn alloc_node_pool_772b50(&mut self) -> Fault<u32> {
        let nodes = self.arena(1, 0x960)?;
        self.init_node_pool_freelist(0x772B50, nodes, 0xA0, 0xF)?;
        Ok(nodes)
    }

    /// 0x31C930: release the tagged block named by `[0x762F90]`, then return the re-read global
    /// in EAX while preserving the arena release's ECX/EDX results.
    pub fn release_mapedit_static_block_762f90(&mut self) -> Fault<TaggedReleaseRegisters> {
        let block = self.get(0x762F90)?;
        let mut registers = Arena {
            memory: &mut *self.memory,
            hooks: &mut *self.hooks,
        }
        .release_tagged_block_with_registers(block)?;
        registers.eax = self.get(0x762F90)?;
        Ok(registers)
    }

    /// 0x31E430: release the tagged record-rows panel block at `[0x7630B0]` and return the
    /// re-read global in EAX while preserving the arena release's ECX/EDX results.
    pub fn release_record_rows_panel_block_7630b0(&mut self) -> Fault<TaggedReleaseRegisters> {
        let block = self.get(0x7630B0)?;
        let mut registers = Arena {
            memory: &mut *self.memory,
            hooks: &mut *self.hooks,
        }
        .release_tagged_block_with_registers(block)?;
        registers.eax = self.get(0x7630B0)?;
        Ok(registers)
    }

    /// 0x2760F0: 0x190-byte block at 0x78B240; the four words 0x78B240..0x78B250 are cleared
    /// first (the first one is then overwritten by the pointer).
    pub fn alloc_table_78b240(&mut self) -> Fault<()> {
        for i in 0..4 {
            self.put(0x78B240 + 4 * i, 0)?;
        }
        self.arena_table(0x190, 0x78B240).map(|_| ())
    }

    /// 0x307CC0: zeroed 0x9600-byte block at 0x784004.
    pub fn alloc_zeroed_784004(&mut self) -> Fault<()> {
        let block = self.arena_table(0x9600, 0x784004)?;
        self.zero_dwords(block, 0x2580)
    }

    /// 0x274650: a 0x100-byte block at 0x74C3B8, then a block of `(signed byte [0x7DE402] * 12
    /// + 0xF) & !0xF` bytes at 0x74C3BC.
    pub fn alloc_tables_74c3b8(&mut self) -> Fault<u32> {
        self.arena_table(0x100, 0x74C3B8)?;
        let mode = self.get(0x7DE400)?;
        let count = ((mode >> 16) as u8 as i8) as i32 as u32;
        let size = count.wrapping_mul(12).wrapping_add(0xF) & 0xFFFF_FFF0;
        self.arena_table(size, 0x74C3BC)
    }

    /// 0x9A0F0(count): clears 0x7B677C, then a `(count * 6 + 0xF) & !0xF` byte block at 0x7B6774.
    pub fn alloc_table_7b6774(&mut self, count: u32) -> Fault<()> {
        let size = count.wrapping_mul(6).wrapping_add(0xF) & 0xFFFF_FFF0;
        self.put(0x7B677C, 0)?;
        self.arena_table(size, 0x7B6774).map(|_| ())
    }

    /// `rep stosd` of zero, forward.
    fn zero_dwords(&mut self, start: u32, dwords: u32) -> Fault<()> {
        for i in 0..dwords {
            self.put(start.wrapping_add(i.wrapping_mul(4)), 0)?;
        }
        Ok(())
    }
    /// Allocate `size` from the ledger, publish it in `global`, zero `dwords` dwords. Returns it.
    fn ledger_zeroed(&mut self, size: u32, global: u32, dwords: u32) -> Fault<u32> {
        let block = self.ledger(size)?;
        self.put(global, block)?;
        self.zero_dwords(block, dwords)?;
        Ok(block)
    }

    /// 0x323E0 `game_alloc_zeroed_689174_0x9000`.
    pub fn alloc_zeroed_689174(&mut self) -> Fault<()> {
        self.ledger_zeroed(0x9000, 0x689174, 0x2400).map(|_| ())
    }

    /// 0x36FC0 `game_alloc_zeroed_68c120_0x4b000`: also publishes the block and its size in
    /// 0x68C54C and 0x68C550.
    pub fn alloc_zeroed_68c120(&mut self) -> Fault<()> {
        self.ledger_zeroed(0x4B000, 0x68C120, 0x12C00)?;
        let block = self.get(0x68C120)?;
        self.put(0x68C54C, block)?;
        self.put(0x68C550, 0x4B000)
    }

    /// 0x37410 `game_alloc_zeroed_68c5e0_0x20000`.
    pub fn alloc_zeroed_68c5e0(&mut self) -> Fault<()> {
        self.ledger_zeroed(0x20000, 0x68C5E0, 0x8000).map(|_| ())
    }

    /// 0x18CA0 `game_alloc_buffers_54ef14_init`: 0x1A000 bytes then a 16-aligned 0x800 byte
    /// buffer, both arena kind 2 (region A). The second size is read back from 0x54EF20.
    pub fn alloc_buffers_54ef14(&mut self) -> Fault<()> {
        self.put(0x54EF20, 0x800)?;
        let first = self.arena(2, 0x1A000)?;
        self.put(0x54EF14, first)?;
        let size = self.get(0x54EF20)?.wrapping_add(0xF) & 0xFFFF_FFF0;
        let second = self.arena(2, size)?;
        self.put(0x54EF18, second)
    }

    /// 0x32A860: allocate the save-slot record block in Region B and publish it at 0x778D38.
    /// The lifted entry requests 0x191530 bytes when game mode 0x79094C is exactly 0x66 and
    /// 0x1530 otherwise, then zeroes 0x54C dwords and repeats zero stores at buffer +4 and +8.
    pub fn alloc_save_slot_buffer_778d38(&mut self) -> Fault<()> {
        let mode = self.get(0x79094C)?;
        let size = if mode == 0x66 { 0x191530 } else { 0x1530 };
        let block = self.arena(1, size)?;
        self.put(0x778D38, block)?;
        self.zero_dwords(block, 0x54C)?;
        self.put(block.wrapping_add(4), 0)?;
        self.put(block.wrapping_add(8), 0)
    }

    /// 0x2FE200: allocate and clear the 0x8000-byte Region-A block at 0x784014.
    pub fn alloc_zeroed_8000_784014(&mut self) -> Fault<()> {
        let block = self.arena(2, 0x8000)?;
        self.put(0x784014, block)?;
        self.zero_dwords(block, 0x2000)
    }

    /// 0x336600, the tail-called Map Maker initializer: allocate and clear its 0x10000-byte
    /// undo buffer, then clear the undo/redo heads and used counter.
    pub fn alloc_mapedit_undo_buffer_765c24(&mut self) -> Fault<()> {
        let block = self.arena(2, 0x10000)?;
        self.put(0x765C24, block)?;
        self.zero_dwords(block, 0x4000)?;
        self.put(0x765C28, 0)?;
        self.put(0x765C30, 0)?;
        self.put(0x765C20, 0)
    }

    /// The full 0x2FE200 -> 0x336600 allocation path, with both blocks and list heads owned by
    /// this readable Rust slice.
    pub fn alloc_mapedit_buffers(&mut self) -> Fault<()> {
        self.alloc_zeroed_8000_784014()?;
        self.alloc_mapedit_undo_buffer_765c24()
    }

    /// 0x1620C0: initialize one 0x18-byte Story objective counter per definition in the active
    /// mode. Reuse a published guest buffer or allocate and publish one at 0x7A2820, clear the
    /// records, then apply the optional definition seed. The guest image continues to own both
    /// definitions and counters until S3.
    pub fn alloc_story_objective_counters_7a2820(&mut self) -> Fault<()> {
        let mode = self.get(0x74C36C)?;
        let count = self.get(mode.wrapping_add(0x78))?;
        if count == 0 {
            return Ok(());
        }

        let definitions = self.get(mode.wrapping_add(0x74))?;
        let published = self.get(0x7A2820)?;
        let block = if published != 0 {
            // Reuse the stable guest buffer when the constructor runs again. The lifted body
            // still clears and reseeds its records; it only skips the arena request/publication.
            published
        } else {
            let bytes = count.wrapping_mul(0x18);
            let block = Arena {
                memory: &mut *self.memory,
                hooks: &mut *self.hooks,
            }
            .allocate_aligned(0x10, bytes, 1)?;
            self.put(0x7A2820, block)?;
            block
        };
        self.zero_dwords(block, count.wrapping_mul(6))?;

        for index in 0..count {
            let definition = definitions.wrapping_add(index.wrapping_mul(0x98));
            if self.get(definition.wrapping_add(0x54))? & 0x8000 != 0 {
                let seed = self.get(definition.wrapping_add(0x64))?;
                let counter = block
                    .wrapping_add(index.wrapping_mul(0x18))
                    .wrapping_add(0x0C);
                self.put(counter, seed)?;
            }
        }
        Ok(())
    }

    /// 0x386E0 `game_messenger_tables_alloc_zeroed`: four ledger blocks. FAITHFUL QUIRK (MEASURED
    /// in the lifted body): the second zero pass runs over the SECOND block (0x68DDC8, 0xAA0 bytes)
    /// for 0x3E8 dwords (0xFA0 bytes), the size of the THIRD block, so it overruns the second block
    /// by 0x500 bytes and the third block (0x68DD18) is never zeroed by this function itself.
    pub fn alloc_messenger_tables(&mut self) -> Fault<()> {
        let roster = self.ledger(0x35A8)?;
        self.put(0x68DDE4, roster)?;
        self.zero_dwords(roster, 0xD6A)?;
        let second = self.ledger(0xAA0)?;
        self.put(0x68DDC8, second)?;
        self.zero_dwords(second, 0x2A8)?;
        let third = self.ledger(0xFA0)?;
        let again = self.get(0x68DDC8)?;
        self.put(0x68DD18, third)?;
        self.zero_dwords(again, 0x3E8)?;
        let fourth = self.ledger(0x2080)?;
        self.put(0x68C6FC, fourth)?;
        self.zero_dwords(fourth, 0x820)
    }

    /// 0x69EE0 `game_alloc_ledger_buffer_6f1e14_0x32000_and_select_6f1e10_when_empty(force)`.
    pub fn ensure_ledger_buffer_6f1e14(&mut self, force: u32) -> Fault<()> {
        let mut buffer = self.get(0x6F1E14)?;
        if force == 0 || buffer == 0 {
            // 0x3E430 returns the ledger flag.
            let enabled = Arena {
                memory: &mut *self.memory,
                hooks: &mut *self.hooks,
            }
            .ledger_flag()?;
            if enabled != 0 {
                buffer = self.ledger(0x32000)?;
                self.put(0x6F1E14, buffer)?;
            } else {
                buffer = self.get(0x6F1E14)?;
            }
            if buffer == 0 {
                return Ok(());
            }
        }
        if self.get(0x6F1E10)? != buffer {
            self.put(0x6F1E10, buffer)?;
            self.put(buffer.wrapping_add(0xC), 0)?;
        }
        Ok(())
    }
}
