// portreg: mirrors=tsfp-engine-object/closure/calls_w8
//! T1908 cutscene state and key-cache slice. Opt-in, Tier 0 for every listed VA.
//! This is the runtime boundary beside the CSINFO/IGCS parsers, not a new format.
use tsfp_engine_memory::{GuestMemory, MemoryFault};
use tsfp_types::GuestAddr;
type Result<T> = core::result::Result<T, MemoryFault>;

/// First signed Story level ID in the guest campaign table.
pub const STORY_CAMPAIGN_LEVEL_IDS: u32 = 0x4d1ac6;
/// The 13 campaign rows place adjacent IDs `0x44` bytes apart.
pub const STORY_CAMPAIGN_LEVEL_ROW_STRIDE: u32 = 0x44;
pub const STORY_CAMPAIGN_LEVEL_COUNT: usize = 13;

/// Object/animation/render callers pass this handle rather than owning a copy of
/// the prop's track pointers and state bytes. It is not a repr(C) host record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CutsceneProp(pub GuestAddr);
/// Shared service for animation consumers, preserving the guest two-slot cache.
pub trait KeyNodeProvider {
    fn key_node(&mut self, frame: i32, node: u32) -> Result<GuestAddr>;
}
pub struct CutsceneState<'a, M: GuestMemory> {
    pub memory: &'a mut M,
}
impl<M: GuestMemory> CutsceneState<'_, M> {
    fn get(&mut self, a: u32) -> Result<u32> {
        self.memory.read_u32(GuestAddr(a))
    }
    fn put(&mut self, a: u32, v: u32) -> Result<()> {
        self.memory.write_u32(GuestAddr(a), v)
    }
    fn word(&mut self, a: u32) -> Result<u16> {
        let mut bytes = [0; 2];
        self.memory.read(GuestAddr(a), &mut bytes)?;
        Ok(u16::from_le_bytes(bytes))
    }
    fn byte(&mut self, a: u32, v: u8) -> Result<()> {
        self.memory.write(GuestAddr(a), &[v])
    }
    /// 0003EE20: invalidate the stream allocation pair.
    // portreg: va=0x0003ee20
    pub fn clear_stream_pair(&mut self) -> Result<()> {
        self.put(0x6b83e0, 0)?;
        self.put(0x6b83e4, 0)
    }
    /// 00058240: remap through a signed 16-bit index, then write the track pair.
    // portreg: va=0x00058240 tier=0
    pub fn set_track_pair(&mut self, index: u32, first: u32, second: u32) -> Result<()> {
        let mapped =
            self.word(0x7e03c0u32.wrapping_add(index.wrapping_mul(2)))? as i16 as i32 as u32;
        let entry = 0x7de660u32.wrapping_add(mapped.wrapping_mul(8));
        self.put(entry, first)?;
        self.put(entry.wrapping_add(4), second)
    }
    /// 00060390: resolve frame data, including four streaming windows and fallback.
    // portreg: va=0x00060390 tier=0
    pub fn frame_pointer(&mut self, frame: u32) -> Result<u32> {
        if frame != 0 {
            let windows = self.get(0x6d7834)?;
            for slot in 0..4u32 {
                let index = self.get(0x7de540 + slot * 4)?;
                let entry = windows.wrapping_add(index.wrapping_mul(8));
                let start = self.get(entry)?;
                if (frame as i32) < (start as i32) {
                    continue;
                }
                let size = self.get(entry.wrapping_add(4))?;
                let start_again = self.get(entry)?;
                if (frame as i32) >= (start_again.wrapping_add(size) as i32) {
                    continue;
                }
                if self.get(0x7de520 + slot * 4)? != 0 {
                    break;
                }
                let header = self.get(0x6d7818)?;
                let stride = self.get(header.wrapping_add(0x14))?;
                let offsets = self.get(0x6d7838)?;
                let relative = self.get(offsets.wrapping_add(frame.wrapping_mul(4)))?;
                let base = self.get(0x7de530)?;
                return Ok(relative
                    .wrapping_add(stride.wrapping_mul(slot))
                    .wrapping_add(base));
            }
        }
        let header = self.get(0x6d7818)?;
        Ok(self.get(header.wrapping_add(4))?.wrapping_add(header))
    }
    /// 00061680: initialize twelve stream words and the two trailing words.
    /// Normal forward string-operation convention (DF=0).
    // portreg: va=0x00061680 tier=0
    pub fn reset_stream_state(&mut self) -> Result<()> {
        for offset in (0..48).step_by(4) {
            self.put(0x6d7840 + offset, 0)?;
        }
        for a in [0x6d785c, 0x6d7868, 0x6d7860, 0x6d786c, 0x6d7864] {
            self.put(a, u32::MAX)?;
        }
        self.put(0x6d7840, 1)?;
        self.put(0x6d7870, 0)?;
        self.put(0x6d7874, 0)
    }
    /// 00061CA0: return the raw state mask, not a normalized boolean.
    pub fn stream_state_mask(&mut self) -> Result<u32> {
        Ok(self.get(0x6d7840)? & 0x10)
    }
    /// Find a cutscene table row by its signed 16-bit key.
    ///
    /// The thirteen rows start at guest address `0x4d1ac0`, have a `0x44` byte
    /// stride, and hold the key at offset `+6`. A missing key returns `-1`.
    /// Keys outside the signed 16-bit range cannot match a row.
    /// Verification: Tier 0 synthetic guest-memory tests.
    pub fn entry_index(&mut self, key: i32) -> Result<i32> {
        for index in 0..STORY_CAMPAIGN_LEVEL_COUNT as u32 {
            if self.word(STORY_CAMPAIGN_LEVEL_IDS + index * STORY_CAMPAIGN_LEVEL_ROW_STRIDE)? as i16
                as i32
                == key
            {
                return Ok(index as i32);
            }
        }
        Ok(-1)
    }

    /// Read the 13 authoritative signed Story level IDs into owned typed data.
    ///
    /// This is the producer for `CampaignLevelTable`: it follows the measured ID field and row
    /// stride in guest memory but does not expose guest pointers or copy unrelated row bytes.
    pub fn story_campaign_level_ids(&mut self) -> Result<[i16; STORY_CAMPAIGN_LEVEL_COUNT]> {
        let mut ids = [0; STORY_CAMPAIGN_LEVEL_COUNT];
        for (index, id) in ids.iter_mut().enumerate() {
            let address = STORY_CAMPAIGN_LEVEL_IDS + index as u32 * STORY_CAMPAIGN_LEVEL_ROW_STRIDE;
            *id = self.word(address)? as i16;
        }
        Ok(ids)
    }
    /// 00064D90: return the matching guest row address or null.
    // portreg: va=0x00064d90
    pub fn entry_pointer(&mut self, key: i32) -> Result<GuestAddr> {
        let index = self.entry_index(key)?;
        Ok(GuestAddr(if index < 0 {
            0
        } else {
            0x4d1ac0 + index as u32 * 0x44
        }))
    }
    /// 00064DD0: choose unsigned word +C or +E; absent row returns zero.
    // portreg: va=0x00064dd0
    pub fn entry_word(&mut self, alternate: u32, key: i32) -> Result<u32> {
        let pointer = self.entry_pointer(key)?.0;
        if pointer == 0 {
            return Ok(0);
        }
        Ok(self.word(pointer + if alternate == 0 { 12 } else { 14 })? as u32)
    }
    /// Save the current sequence at `0x7de310`, then publish `selection` at
    /// `0x7de314`. Consecutive calls retain only the immediate predecessor.
    /// Verification: Tier 0 synthetic guest-memory tests.
    pub fn select_sequence(&mut self, selection: u32) -> Result<()> {
        let old = self.get(0x7de314)?;
        self.put(0x7de310, old)?;
        self.put(0x7de314, selection)
    }
    // portreg: va=0x000665b0
    /// 000665B0.
    pub fn clear_sequence_gate(&mut self) -> Result<()> {
        self.put(0x6f11bc, 0)
    }
    /// 000665C0.
    pub fn sequence_gate_clear(&mut self) -> Result<bool> {
        Ok(self.get(0x6f11bc)? == 0)
    }
    /// 0006EE00.
    // portreg: va=0x0006f430 tier=0
    // portreg: va=0x0006ee00
    pub fn invalidate_subtitle(&mut self) -> Result<()> {
        self.put(0x7bb248, u32::MAX)
    }
    /// 0006F430: twenty rows; signed out-of-range returns -1 without table access.
    pub fn slot_word(&mut self, index: i32) -> Result<u32> {
        if !(0..20).contains(&index) {
            return Ok(u32::MAX);
        }
        self.get(0x51fc48 + index as u32 * 36)
    }
    /// 0006F4B0: publish two selected-slot globals.
    // portreg: va=0x0006f4b0
    pub fn set_slot_pair(&mut self, first: u32, second: u32) -> Result<()> {
        self.put(0x6f73d0, first)?;
        self.put(0x6f73d4, second)
    }
    /// 0008E970: clear prop tracks/state, retaining unrelated bytes.
    // portreg: va=0x0008e970 tier=0
    pub fn reset_prop(&mut self, prop: CutsceneProp) -> Result<()> {
        let p = prop.0.0;
        for offset in [0x1b8, 0x1ec, 0x1f0] {
            self.put(p.wrapping_add(offset), 0)?;
        }
        self.memory
            .write(GuestAddr(p.wrapping_add(0x1f6)), &0u16.to_le_bytes())?;
        for (offset, value) in [
            (0x1f8, 255),
            (0x1f9, 255),
            (0x1fa, 0),
            (0x87, 0),
            (0x1fb, 0),
            (0x1fc, 0),
            (0x1fd, 0),
        ] {
            self.byte(p.wrapping_add(offset), value)?;
        }
        self.memory
            .write(GuestAddr(p.wrapping_add(0x7c)), &u16::MAX.to_le_bytes())
    }
    /// 00094220: the guest's third argument (+C) is unused. Arguments at +8,
    /// +10,+14 set +1F8,+1F9,+1FA; mutation order is +1F8,+1FA,+1F9.
    // portreg: va=0x00094220
    pub fn set_prop_state(
        &mut self,
        prop: CutsceneProp,
        first: u8,
        _unused: u32,
        second: u8,
        third: u8,
    ) -> Result<()> {
        let p = prop.0.0;
        self.byte(p.wrapping_add(0x1f8), first)?;
        self.byte(p.wrapping_add(0x1fa), third)?;
        self.byte(p.wrapping_add(0x1f9), second)?;
        let record = self.get(p)?;
        let flags = self.get(record.wrapping_add(0x18))?;
        self.put(record.wrapping_add(0x18), flags | 4)
    }
    /// 00098DA0.
    // portreg: va=0x00098da0
    pub fn active_record(&mut self) -> Result<GuestAddr> {
        Ok(GuestAddr(self.get(0x70e164)?))
    }
    /// 00098DB0: no active record means no writes.
    // portreg: va=0x00098db0 tier=0
    pub fn request_stop(&mut self) -> Result<()> {
        let record = self.get(0x70e164)?;
        if record != 0 {
            self.put(record.wrapping_add(0x1c), 1)?;
            self.put(0x4d977c, 0)?;
        }
        Ok(())
    }
    /// 000991C0: reset time and flags using the current frame stamp.
    /// The original requires a live active record; null is not silently accepted.
    pub fn restart_active(&mut self) -> Result<()> {
        let record = self.get(0x70e164)?;
        self.put(record.wrapping_add(8), 0)?;
        let stamp = self.get(0x7497a8)?;
        let record = self.get(0x70e164)?;
        self.put(record.wrapping_add(4), stamp)?;
        for a in [0x70e158, 0x70e160, 0x7b67c8] {
            self.put(a, 0)?;
        }
        Ok(())
    }
    /// 001395B0.
    pub fn invalidate_sequence_index(&mut self) -> Result<()> {
        self.put(0x7490f0, u32::MAX)
    }
    /// 0023EF00.
    pub fn current_sequence(&mut self) -> Result<u32> {
        self.get(0x78cd58)
    }
    /// 0023EFC0.
    // portreg: va=0x0023efc0
    pub fn reset_sequence_selection(&mut self) -> Result<()> {
        self.put(0x78cd58, u32::MAX)?;
        self.put(0x75ce58, u32::MAX)?;
        self.put(0x78cd54, 0)?;
        self.put(0x75ce5c, 0)
    }
    /// 00338170: unchecked stride-24 table field +4, guest wrapping index.
    pub fn file_entry_field(&mut self, index: u32) -> Result<u32> {
        self.get(0x52ec8cu32.wrapping_add(index.wrapping_mul(24)))
    }
    /// `0x60410` result plus the live EDX effect at its register boundary.
    /// Cache hits preserve the incoming EDX; the slow path leaves the offset-table pointer.
    pub fn key_node_register_effects(
        &mut self,
        frame: i32,
        node: u32,
    ) -> Result<(GuestAddr, Option<u32>)> {
        let streaming = self.get(0x7de530)?;
        let frame = if streaming == 0 || frame < 0 {
            0
        } else {
            frame
        };
        let count = self.get(0x7bb260)? as i32;
        let frame = if frame >= count {
            count.wrapping_sub(1)
        } else {
            frame
        };
        let (first, second);
        if self.get(0x4d19cc)? == node {
            first = self.get(0x4d19d0)? as i32;
            if frame == first {
                return Ok((GuestAddr(self.get(0x6d782c)?), None));
            }
            second = self.get(0x4d19d4)? as i32;
            if frame == second {
                return Ok((GuestAddr(self.get(0x6d7830)?), None));
            }
        } else {
            first = -1000;
            second = -1000;
            self.put(0x4d19d0, first as u32)?;
            self.put(0x4d19d4, second as u32)?;
            self.put(0x4d19cc, node)?;
        }
        let base = self.frame_pointer(frame as u32)?;
        let stride = self.get(0x7bb284)?;
        let offsets = self.get(0x6d783c)?;
        let index = stride.wrapping_mul(frame as u32).wrapping_add(node);
        let pointer = base.wrapping_add(self.get(offsets.wrapping_add(index.wrapping_mul(4)))?);
        if (frame > first && first < second) || (frame < first && first < second) {
            self.put(0x4d19d0, frame as u32)?;
            self.put(0x6d782c, pointer)?;
        } else {
            self.put(0x4d19d4, frame as u32)?;
            self.put(0x6d7830, pointer)?;
        }
        Ok((GuestAddr(pointer), Some(offsets)))
    }
}
impl<M: GuestMemory> KeyNodeProvider for CutsceneState<'_, M> {
    /// 00060410: clamp frame, consult two cache slots, resolve and replace a slot.
    // portreg: va=0x00060410
    fn key_node(&mut self, frame: i32, node: u32) -> Result<GuestAddr> {
        self.key_node_register_effects(frame, node)
            .map(|(pointer, _)| pointer)
    }
}
