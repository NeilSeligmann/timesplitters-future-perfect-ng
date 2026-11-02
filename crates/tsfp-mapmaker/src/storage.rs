//! mapmaker/storage (T1899, batch 1: the small leaves). Tier 0, confidence INFERRED (static
//! disassembly only). Not yet ported (verification debt): 0x190530 0x190620 0x1908a0 0x190900
//! 0x3006c0 0x301a90 0x312df0 0x336640 0x337760. Hints for the big one: see docs/t1717-mapmaker-format.md.
use tsfp_gameplay::memory::GuestMemory;

/// Save-signature scratch buffer (0x5651a8, 0x400 bytes) and its length word (0x5655a8).
pub const SIGNATURE_BUFFER: u32 = 0x0056_51a8;
pub const SIGNATURE_LENGTH: u32 = 0x0056_55a8;
pub const SIGNATURE_MAX: u32 = 0x400;
/// Pointer to the mapedit state (nodes at +0x81a8, stride 0xb4, count at +0x81a4).
pub const MAPEDIT_STATE_PTR: u32 = 0x0078_44a8;
pub const NODE_TABLE_OFFSET: u32 = 0x81a8;
pub const NODE_COUNT_OFFSET: u32 = 0x81a4;
pub const NODE_STRIDE: u32 = 0xb4;
/// Static mapedit data returned by 0x311bb0.
pub const MAPEDIT_TABLE_52D464: u32 = 0x0052_d464;
/// Flag byte read by 0x32a3a0.
pub const FLAG_BYTE_765A80: u32 = 0x0076_5a80;

/// Callee outside the slice used by the save signature size helper.
pub trait StorageHost {
    /// 0x37e89e, called with the size class built by `signature_size_from_flags`.
    fn signature_size_lookup(&mut self, size_class: u32) -> u32;
}

/// 0x25210 `game_buffer_5651a8_set`: copy `len` bytes from `src` into the signature buffer and
/// record the length. Does nothing when `len` exceeds 0x400.
pub fn signature_buffer_set(m: &mut impl GuestMemory, src: u32, len: u32) {
    if len > SIGNATURE_MAX {
        return;
    }
    for i in 0..len {
        let byte = m.read8(src.wrapping_add(i));
        m.write8(SIGNATURE_BUFFER + i, byte);
    }
    m.write32(SIGNATURE_LENGTH, len);
}

/// 0x25240 `game_signature_size_from_flags`: bit 3 gives 1, then bit 30 adds 4 else bit 2 adds 8.
pub fn signature_size_from_flags(flags: u32, h: &mut impl StorageHost) -> u32 {
    let mut class = u32::from(flags & 8 != 0);
    if flags & 0x4000_0000 != 0 {
        class |= 4;
    } else if flags & 4 != 0 {
        class |= 8;
    }
    h.signature_size_lookup(class)
}

/// 0x311bb0 `game_address_of_52d464`.
pub fn address_of_52d464() -> u32 {
    MAPEDIT_TABLE_52D464
}

/// 0x3082d0: count list nodes (next pointer at +0x14) whose first dword equals `key`; a negative
/// `key` counts every node.
pub fn count_nodes_with_first_dword(m: &impl GuestMemory, head: u32, key: i32) -> u32 {
    let mut count = 0;
    let mut node = head;
    while node != 0 {
        if key < 0 || m.read32(node) as i32 == key {
            count += 1;
        }
        node = m.read32(node + 0x14);
    }
    count
}

/// 0x31dcd0: pack four flag-struct fields into a bitmask: bit 4 when +0x10 is non-zero, bit 2
/// when +0xc is non-zero, bit 8 when +4 is non-zero and bit 1 when +8 is non-zero.
pub fn pack_flag_struct(m: &impl GuestMemory, record: u32) -> u32 {
    let bit = |off: u32, mask: u32| if m.read32(record + off) != 0 { mask } else { 0 };
    bit(0x10, 4) | bit(0xc, 2) | bit(4, 8) | bit(8, 1)
}

/// 0x322150: find the node record whose halfword at +0xc equals `value` (sign extended), returning
/// the record address or 0. (The name in the table says index, the code returns the pointer.)
pub fn find_node_by_halfword(m: &impl GuestMemory, value: i32) -> u32 {
    let state = m.read32(MAPEDIT_STATE_PTR);
    let count = m.read32(state + NODE_COUNT_OFFSET) as i32;
    let mut rec = state + NODE_TABLE_OFFSET;
    for _ in 0..count.max(0) {
        if i32::from(m.read16(rec + 0xc) as i16) == value {
            return rec;
        }
        rec += NODE_STRIDE;
    }
    0
}

/// 0x32a3a0: bit 1 of the flag byte at 0x765a80.
pub fn flag_bit_1(m: &impl GuestMemory) -> u32 {
    u32::from(m.read8(FLAG_BYTE_765A80) >> 1 & 1)
}

/// 0x3346e0: both halfwords of the pair are at least 0x3e9.
pub fn both_halfwords_at_least_0x3e9(m: &impl GuestMemory, pair: u32) -> bool {
    m.read16(pair) >= 0x3e9 && m.read16(pair + 2) >= 0x3e9
}

/// 0x334700: zero two words.
pub fn zero_two_words(m: &mut impl GuestMemory, dst: u32) {
    m.write32(dst, 0);
    m.write32(dst + 4, 0);
}

/// Selected slot globals and mapedit buffers.
pub const MAPEDIT_UNDO_BUFFER_PTR: u32 = 0x0076_5c24;
pub const MAPEDIT_LEVEL_BUFFER_PTR: u32 = 0x0076_5e38;
pub const LEVEL_CHUNK_TAG: u32 = 0x00c7_7668;
pub const LABEL_FORMAT_SELECTOR: u32 = 0x004c_0454;
pub const SAVE_SLOT_DIRTY_FLAG: u32 = 0x0076_2cbc;
pub const SLOT_TYPE_LABEL: u32 = 0xa;

/// Callees outside the slice used by the mapmaker save flows.
pub trait MapmakerSaveHost {
    /// 0x827b0 set the save UI mode (0 enumerate, 1 verify, 2 write, 3 label).
    fn fn_000827b0_set_mode(&mut self, mode: u32);
    /// 0x827f0 the selected device slot.
    fn selected_slot(&mut self, m: &dyn GuestMemory) -> u32;
    /// 0x35e960 net live mode active.
    fn net_is_live_mode_active(&mut self) -> bool;
    /// 0x25280 save file load verify (slot, name, label, buffer, size, flags) -> status.
    fn fn_00025280_load_verify(
        &mut self,
        slot: u32,
        name: u32,
        label: u32,
        buffer: u32,
        size: u32,
        flags: u32,
    ) -> u32;
    /// 0x32a5b0 find the chunk with `tag` in a buffer.
    fn fn_0032a5b0_find_chunk(&mut self, buffer: u32, tag: u32) -> u32;
    /// 0x3c85d4 sprintf the label format into a local buffer.
    fn sprintf_label(&mut self, m: &mut dyn GuestMemory, dst: u32, format: u32, arg: u32);
    /// 0x25670 delete the save game named by the guest string at `name` on `slot`.
    fn fn_00025670_save_game_delete(
        &mut self,
        m: &mut dyn GuestMemory,
        slot: u32,
        name: u32,
    ) -> u32;
    /// 0x25e40 / 0x25ff0 enumerate saves (`content` selects the content flavour).
    fn find_enumerate(&mut self, content: bool, slot: u32, label: u32, callback: u32, context: u32);
    /// 0x3006c0 serialize the map state into a chunk buffer with a mode.
    fn fn_003006c0_serialize(&mut self, buffer: u32, mode: u32);
    /// 0x658d0 enter a state with a 0xa0 byte request record at guest address `request`.
    fn fn_000658d0_enter_state(&mut self, request: u32);
    /// 0x77470 follow the widget chain to its leaf.
    fn fn_00077470_widget_leaf(&mut self, widget: u32) -> u32;
    /// 0x337600 refresh helper (arg, 0).
    fn fn_00337600_refresh(&mut self, arg: u32);
    /// 0x78980 rebuild a widget.
    fn fn_00078980_rebuild_widget(&mut self, widget: u32);
    /// 0x1504f0 crc32 of `length` bytes at `buffer`.
    fn fn_001504f0_crc32(&mut self, m: &mut dyn GuestMemory, buffer: u32, length: u32) -> u32;
    /// 0x25240 signature size from the save flags.
    fn fn_00025240_signature_size(&mut self, flags: u32) -> u32;
    /// 0x25710 verify an appended signature: (buffer, size, flags) -> status, data length in `out`.
    fn fn_00025710_signature_verify(
        &mut self,
        m: &mut dyn GuestMemory,
        buffer: u32,
        size: u32,
        out: u32,
        flags: u32,
    ) -> u32;
    /// 0x26740 content write (slot, label, name, hash, buffer, size, flags).
    fn fn_00026740_content_write(
        &mut self,
        m: &mut dyn GuestMemory,
        slot: u32,
        label: u32,
        name: u32,
        hash: u32,
        buffer: u32,
        size: u32,
        flags: u32,
    ) -> u32;
    /// 0x263e0 save game write (slot, label, name, buffer, size, flags).
    fn fn_000263e0_save_game_write(
        &mut self,
        m: &mut dyn GuestMemory,
        slot: u32,
        label: u32,
        name: u32,
        buffer: u32,
        size: u32,
        flags: u32,
    ) -> u32;
}

/// 0x190530 `game_mapmaker_anicemap_save_verify`. Returns the status of the load verify.
pub fn anicemap_save_verify(
    m: &mut impl GuestMemory,
    h: &mut impl MapmakerSaveHost,
    record: u32,
    buffer: u32,
) -> u32 {
    h.fn_000827b0_set_mode(1);
    let mut flags = if h.selected_slot(m) == SLOT_TYPE_LABEL {
        0xe
    } else {
        2
    };
    if !h.net_is_live_mode_active() {
        flags &= !4;
    }
    let name = record.wrapping_add(0x20);
    let slot = h.selected_slot(m);
    let status = h.fn_00025280_load_verify(slot, name, 0x0047_df80, buffer, 0x7fe0, flags);
    if status != 1 {
        return status;
    }
    let chunk = h.fn_0032a5b0_find_chunk(buffer, LEVEL_CHUNK_TAG);
    let size = m.read16(chunk + 0x248).wrapping_add(4);
    m.write8(record + 0x66, size as u8);
    m.write8(record + 0x67, (size >> 8) as u8);
    if flags & 4 != 0 {
        m.write8(chunk + 8, 1);
        m.write32(SAVE_SLOT_DIRTY_FLAG, 1);
        return status;
    }
    if m.read8(chunk + 8) != 0 && h.net_is_live_mode_active() {
        let slot = h.selected_slot(m);
        let again =
            h.fn_00025280_load_verify(slot, name, 0x0047_df80, buffer, 0x7fe0, flags | 0x14);
        if again != status {
            return again;
        }
        m.write32(SAVE_SLOT_DIRTY_FLAG, status);
        return status;
    }
    m.write32(SAVE_SLOT_DIRTY_FLAG, 0);
    status
}

/// 0x190620 `game_mapmaker_label_format`: format the label and delete the save of that name.
pub fn mapmaker_label_format(
    m: &mut impl GuestMemory,
    h: &mut impl MapmakerSaveHost,
    scratch: u32,
    arg: u32,
) -> u32 {
    h.fn_000827b0_set_mode(3);
    let format = if m.read32(LABEL_FORMAT_SELECTOR) == 1 {
        0x0047_df90
    } else {
        0x0047_df9c
    };
    h.sprintf_label(m, scratch, format, arg);
    let slot = h.selected_slot(m);
    h.fn_00025670_save_game_delete(m, slot, scratch)
}

/// 0x1908a0 `game_mapmaker_enumerate_anicemap_saves`: reset the save list record and enumerate.
pub fn enumerate_anicemap_saves(m: &mut impl GuestMemory, h: &mut impl MapmakerSaveHost) -> u32 {
    h.fn_000827b0_set_mode(0);
    let list = m.read32(0x0077_8d38);
    m.write32(list + 4, 2);
    m.write32(list + 8, 0);
    let slot = h.selected_slot(m);
    h.find_enumerate(slot == SLOT_TYPE_LABEL, slot, 0x0047_df80, 0x0019_0720, 0);
    1
}

/// 0x336640 clear the undo (0x4000 dwords) and level (0x2000 dwords) chunk buffers and the three
/// counters; when `serialize` is set, serialize a baseline into the level buffer (mode 5).
pub fn clear_undo_and_level_chunk_buffers(
    m: &mut impl GuestMemory,
    h: &mut impl MapmakerSaveHost,
    serialize: bool,
) {
    let level = m.read32(MAPEDIT_LEVEL_BUFFER_PTR);
    let undo = m.read32(MAPEDIT_UNDO_BUFFER_PTR);
    for i in 0..0x4000u32 {
        m.write32(undo.wrapping_add(i * 4), 0);
    }
    for i in 0..0x2000u32 {
        m.write32(level.wrapping_add(i * 4), 0);
    }
    for counter in [0x0076_5c20u32, 0x0076_5c28, 0x0076_5c30] {
        m.write32(counter, 0);
    }
    if serialize {
        h.fn_003006c0_serialize(level, 5);
    }
}

/// 0x312df0 serialize the level buffer, clear the chunk buffers and enter state code 0x65.
pub fn serialize_level_buffer_and_enter_state_0x65(
    m: &mut impl GuestMemory,
    h: &mut impl MapmakerSaveHost,
    request: u32,
) {
    let buffer = m.read32(0x0078_4014);
    h.fn_003006c0_serialize(buffer, 3);
    clear_undo_and_level_chunk_buffers(m, h, false);
    for i in 0..0x28u32 {
        m.write32(request + i * 4, 0);
    }
    m.write8(request + 0x54, 0x65);
    h.fn_000658d0_enter_state(request);
    m.write32(0x0078_407c, 0);
    m.write32(0x0076_2f74, 0);
}

/// 0x337760 when the dirty flag (0x20 at state+0x238) is set: rebuild the active widget if it is
/// the mapedit one, then clear the flag.
pub fn save_snapshot_when_dirty(
    m: &mut impl GuestMemory,
    h: &mut impl MapmakerSaveHost,
    argument: u32,
) {
    let state = m.read32(MAPEDIT_STATE_PTR);
    if m.read8(state + 0x238) & 0x20 == 0 {
        return;
    }
    let widget_index = m.read32(0x007b_a9e0).wrapping_mul(0x1e0);
    let widget = m.read32(widget_index.wrapping_add(0x007b_a26c));
    let leaf = h.fn_00077470_widget_leaf(widget);
    h.fn_00337600_refresh(argument);
    if leaf != 0 && m.read32(leaf + 4) == address_of_52d464() {
        h.fn_00078980_rebuild_widget(leaf);
    }
    let state = m.read32(MAPEDIT_STATE_PTR);
    let flags = m.read32(state + 0x238) & !0x20;
    m.write32(state + 0x238, flags);
}

/// 0x190900 `game_mapmaker_anicemap_write`: write the chunk buffer as an anicemap save (content
/// write on the device-10 slot, save game write otherwise), appending a signature when the
/// signature flag byte is set. `name` is the save name string, `buffer` the 0x7fe0 byte level
/// buffer and `mode` selects bit 0 (zero adds nothing). Parameter positions follow the stack
/// slots the original reads (the first and fourth stack arguments are not used).
pub fn anicemap_write(
    m: &mut impl GuestMemory,
    h: &mut impl MapmakerSaveHost,
    scratch: u32,
    buffer: u32,
    name: u32,
    mode: u32,
) -> u32 {
    let chunk = h.fn_0032a5b0_find_chunk(buffer, LEVEL_CHUNK_TAG);
    let mut flags = u32::from(mode == 0);
    let mut length = 0;
    while m.read8(name.wrapping_add(length)) != 0 {
        length += 1;
    }
    let crc = h.fn_001504f0_crc32(m, name, length) ^ m.read32(name.wrapping_add(0x60));
    let format = if m.read32(LABEL_FORMAT_SELECTOR) == 1 {
        0x0047_df90
    } else {
        0x0047_df9c
    };
    h.sprintf_label(m, scratch, format, name);
    h.fn_000827b0_set_mode(2);
    flags |= 2;
    if m.read8(chunk + 8) != 0 {
        flags |= 4;
    }
    let mut size_class = 0;
    if h.selected_slot(m) == SLOT_TYPE_LABEL {
        flags |= 0xc;
        size_class = h.fn_00025240_signature_size(flags);
    }
    let slot = h.selected_slot(m);
    if slot == SLOT_TYPE_LABEL {
        return h.fn_00026740_content_write(
            m,
            slot,
            0x0047_df80,
            scratch,
            crc,
            buffer,
            size_class.wrapping_add(0x7fe0),
            flags,
        );
    }
    if flag_bit_1(m) != 0 {
        flags |= 0x24;
        size_class = h.fn_00025240_signature_size(flags);
        let out = scratch.wrapping_add(0x40);
        let status =
            h.fn_00025710_signature_verify(m, buffer, size_class.wrapping_add(0x7fe0), out, flags);
        if status != 1 {
            return status;
        }
        let data_len = m.read32(out);
        signature_buffer_set(m, buffer.wrapping_add(data_len), size_class);
    }
    let slot = h.selected_slot(m);
    h.fn_000263e0_save_game_write(m, slot, 0x0047_df80, scratch, buffer, 0x7fe0, flags)
}

#[cfg(test)]
#[path = "test_memory.rs"]
mod test_memory;

#[cfg(test)]
mod tests {
    use super::test_memory::Mem;
    use super::*;

    impl Mem {
        fn write16_for_test(&mut self, a: u32, v: u16) {
            self.write8(a, v as u8);
            self.write8(a + 1, (v >> 8) as u8);
        }
    }
    struct Host;
    impl StorageHost for Host {
        fn signature_size_lookup(&mut self, c: u32) -> u32 {
            c + 100
        }
    }

    #[test]
    fn signature() {
        let mut m = Mem::default();
        for i in 0..6 {
            m.write8(0x100 + i, i as u8 + 1);
        }
        signature_buffer_set(&mut m, 0x100, 6);
        assert_eq!(
            (m.read8(SIGNATURE_BUFFER + 5), m.read32(SIGNATURE_LENGTH)),
            (6, 6)
        );
        signature_buffer_set(&mut m, 0x100, 0x401);
        assert_eq!(m.read32(SIGNATURE_LENGTH), 6);
        assert_eq!(signature_size_from_flags(0, &mut Host), 100);
        assert_eq!(
            signature_size_from_flags(8 | 0x4000_0000 | 4, &mut Host),
            105
        );
        assert_eq!(signature_size_from_flags(8 | 4, &mut Host), 109);
        assert_eq!(address_of_52d464(), 0x52d464);
    }

    #[test]
    fn lists_and_flags() {
        let mut m = Mem::default();
        m.write32(0x200, 5);
        m.write32(0x214, 0x300);
        m.write32(0x300, 7);
        assert_eq!(count_nodes_with_first_dword(&m, 0x200, 5), 1);
        assert_eq!(count_nodes_with_first_dword(&m, 0x200, -1), 2);
        assert_eq!(count_nodes_with_first_dword(&m, 0, -1), 0);
        m.write32(0x400 + 8, 1);
        m.write32(0x400 + 0x10, 1);
        assert_eq!(pack_flag_struct(&m, 0x400), 5);
        m.write8(FLAG_BYTE_765A80, 2);
        assert_eq!(flag_bit_1(&m), 1);
        m.write32(0x500, 0x03e9_03e9);
        assert!(both_halfwords_at_least_0x3e9(&m, 0x500));
        m.write32(0x500, 0x03e8_03e9);
        assert!(!both_halfwords_at_least_0x3e9(&m, 0x500));
        zero_two_words(&mut m, 0x500);
        assert_eq!(m.read32(0x500), 0);
    }

    #[test]
    fn node_lookup() {
        let mut m = Mem::default();
        m.write32(MAPEDIT_STATE_PTR, 0x1000);
        m.write32(0x1000 + NODE_COUNT_OFFSET, 2);
        m.write32(0x1000 + NODE_TABLE_OFFSET + NODE_STRIDE + 0xc, 0xffff);
        let want = 0x1000 + NODE_TABLE_OFFSET + NODE_STRIDE;
        assert_eq!(find_node_by_halfword(&m, -1), want);
        assert_eq!(find_node_by_halfword(&m, 3), 0);
    }

    #[derive(Default)]
    struct SaveHost {
        log: Vec<(u32, u32)>,
        selected: u32,
        live: bool,
        verify: Vec<u32>,
        leaf: u32,
    }
    impl MapmakerSaveHost for SaveHost {
        fn fn_000827b0_set_mode(&mut self, mode: u32) {
            self.log.push((0x827b0, mode));
        }
        fn selected_slot(&mut self, _m: &dyn GuestMemory) -> u32 {
            self.selected
        }
        fn net_is_live_mode_active(&mut self) -> bool {
            self.live
        }
        fn fn_00025280_load_verify(
            &mut self,
            _s: u32,
            _n: u32,
            _l: u32,
            _b: u32,
            _z: u32,
            f: u32,
        ) -> u32 {
            self.log.push((0x25280, f));
            self.verify.remove(0)
        }
        fn fn_0032a5b0_find_chunk(&mut self, _b: u32, _t: u32) -> u32 {
            0x8000
        }
        fn sprintf_label(&mut self, _m: &mut dyn GuestMemory, _d: u32, f: u32, a: u32) {
            self.log.push((0x3c85d4, f + a));
        }
        fn fn_00025670_save_game_delete(
            &mut self,
            _m: &mut dyn GuestMemory,
            s: u32,
            _n: u32,
        ) -> u32 {
            s + 100
        }
        fn find_enumerate(&mut self, c: bool, _s: u32, _l: u32, _cb: u32, _x: u32) {
            self.log.push((0x25e40, u32::from(c)));
        }
        fn fn_003006c0_serialize(&mut self, b: u32, mode: u32) {
            self.log.push((0x3006c0, b + mode));
        }
        fn fn_000658d0_enter_state(&mut self, r: u32) {
            self.log.push((0x658d0, r));
        }
        fn fn_00077470_widget_leaf(&mut self, _w: u32) -> u32 {
            self.leaf
        }
        fn fn_00337600_refresh(&mut self, a: u32) {
            self.log.push((0x337600, a));
        }
        fn fn_00078980_rebuild_widget(&mut self, w: u32) {
            self.log.push((0x78980, w));
        }
        fn fn_001504f0_crc32(&mut self, _m: &mut dyn GuestMemory, _b: u32, l: u32) -> u32 {
            0x100 + l
        }
        fn fn_00025240_signature_size(&mut self, f: u32) -> u32 {
            f * 16
        }
        fn fn_00025710_signature_verify(
            &mut self,
            m: &mut dyn GuestMemory,
            _b: u32,
            _s: u32,
            out: u32,
            _f: u32,
        ) -> u32 {
            m.write8(out, 5);
            self.verify.remove(0)
        }
        fn fn_00026740_content_write(
            &mut self,
            _m: &mut dyn GuestMemory,
            _s: u32,
            _l: u32,
            _n: u32,
            hash: u32,
            _b: u32,
            size: u32,
            f: u32,
        ) -> u32 {
            self.log.push((0x26740, hash + size + f));
            1
        }
        fn fn_000263e0_save_game_write(
            &mut self,
            _m: &mut dyn GuestMemory,
            _s: u32,
            _l: u32,
            _n: u32,
            _b: u32,
            size: u32,
            f: u32,
        ) -> u32 {
            self.log.push((0x263e0, size + f));
            2
        }
    }

    #[test]
    fn save_flows() {
        let mut m = Mem::default();
        let mut h = SaveHost {
            selected: 3,
            verify: vec![1, 1],
            ..Default::default()
        };
        m.write16_for_test(0x8000 + 0x248, 10);
        assert_eq!(anicemap_save_verify(&mut m, &mut h, 0x100, 0x200), 1);
        assert_eq!(m.read16(0x100 + 0x66), 14);
        assert_eq!(m.read32(SAVE_SLOT_DIRTY_FLAG), 0);
        assert_eq!(h.log, vec![(0x827b0, 1), (0x25280, 2 & !4)]);
        h.log.clear();
        h.selected = 0xa;
        h.live = true;
        h.verify = vec![1];
        assert_eq!(anicemap_save_verify(&mut m, &mut h, 0x100, 0x200), 1);
        assert_eq!((m.read8(0x8008), m.read32(SAVE_SLOT_DIRTY_FLAG)), (1, 1));
        h.verify = vec![1, 7];
        h.selected = 3;
        assert_eq!(anicemap_save_verify(&mut m, &mut h, 0x100, 0x200), 7);
        assert_eq!(mapmaker_label_format(&mut m, &mut h, 0x300, 4), 103);
        assert_eq!(enumerate_anicemap_saves(&mut m, &mut h), 1);
    }

    #[test]
    fn buffers_and_snapshot() {
        let mut m = Mem::default();
        let mut h = SaveHost::default();
        m.write32(MAPEDIT_LEVEL_BUFFER_PTR, 0x20000);
        m.write32(MAPEDIT_UNDO_BUFFER_PTR, 0x40000);
        m.write32(0x20000 + 0x1ffc, 9);
        m.write32(0x765c20, 5);
        clear_undo_and_level_chunk_buffers(&mut m, &mut h, true);
        assert_eq!((m.read32(0x20000 + 0x1ffc), m.read32(0x765c20)), (0, 0));
        assert_eq!(h.log, vec![(0x3006c0, 0x20005)]);
        h.log.clear();
        m.write32(0x784014, 0x500);
        serialize_level_buffer_and_enter_state_0x65(&mut m, &mut h, 0x9000);
        assert_eq!(m.read8(0x9054), 0x65);
        assert!(h.log.contains(&(0x658d0, 0x9000)));
        m.write32(MAPEDIT_STATE_PTR, 0x7000);
        m.write8(0x7000 + 0x238, 0x20);
        m.write32(0x7ba9e0, 0);
        m.write32(0x7ba26c, 0x60);
        h.leaf = 0x6100;
        m.write32(0x6100 + 4, MAPEDIT_TABLE_52D464);
        h.log.clear();
        save_snapshot_when_dirty(&mut m, &mut h, 77);
        assert_eq!(h.log, vec![(0x337600, 77), (0x78980, 0x6100)]);
        assert_eq!(m.read8(0x7000 + 0x238), 0);
    }

    #[test]
    fn anicemap_write_paths() {
        let mut m = Mem::default();
        let mut h = SaveHost {
            selected: 3,
            ..Default::default()
        };
        m.write8(0x400, b'a');
        m.write32(0x400 + 0x60, 0x55);
        // plain save game write: flags = 1 (mode 0) | 2
        assert_eq!(anicemap_write(&mut m, &mut h, 0x900, 0x2000, 0x400, 0), 2);
        assert_eq!(h.log.last(), Some(&(0x263e0, 0x7fe0 + 3)));
        // content slot: size class from the flags (0xc|2|1... ) and crc xor
        h.selected = 0xa;
        anicemap_write(&mut m, &mut h, 0x900, 0x2000, 0x400, 1);
        let flags = 2 | 0xc;
        assert_eq!(
            h.log.last(),
            Some(&(0x26740, (0x101 ^ 0x55) + flags * 16 + 0x7fe0 + flags))
        );
        // signature flag with a failing verify returns its status
        h.selected = 3;
        m.write8(0x765a80, 2);
        h.verify = vec![9];
        assert_eq!(anicemap_write(&mut m, &mut h, 0x900, 0x2000, 0x400, 1), 9);
        h.verify = vec![1];
        assert_eq!(anicemap_write(&mut m, &mut h, 0x900, 0x2000, 0x400, 1), 2);
        assert_eq!(m.read32(SIGNATURE_LENGTH), (2 | 0x24) * 16);
    }
}
