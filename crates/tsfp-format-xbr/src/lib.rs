//! Level root envelope, distinct from streamed/model XBR variants.
use tsfp_format_core::*;
pub const POINTERS: &[usize] = &[
    0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 48, 52, 56, 60, 64, 68, 0x60, 0x64, 0x68, 0x6c,
    0x78, 0x7c, 0x80, 0x84, 0x88, 0x8c, 0xa8, 0xac, 0xb0, 0xb4, 0xbc, 0xc0, 0xc4,
];
#[derive(Debug, Clone)]
pub struct Room {
    pub offset: usize,
    pub words: Vec<u32>,
}
/// A retained level-root envelope with editable root and room words.
///
/// Pointer fields in `rooms` are relative file offsets. `relocated` returns
/// rebased copies; it does not alter the retained bytes used by `encode`.
///
/// ```
/// use tsfp_format_xbr::Level;
/// let mut bytes = vec![0; 232];
/// bytes[4..8].copy_from_slice(&48u32.to_le_bytes());
/// let level = Level::parse(&bytes).unwrap();
/// assert!(level.rooms.is_empty());
/// assert_eq!(level.encode().unwrap(), bytes);
/// ```
#[derive(Debug, Clone)]
pub struct Level {
    pub header: Vec<u32>,
    pub rooms: Vec<Room>,
    original: Vec<u8>,
    room_offsets: Vec<usize>,
}
impl Level {
    pub fn parse(b: &[u8]) -> Result<Self> {
        let header = words(b, 0, 12)?;
        let table = header[1] as usize;
        if table < 48 {
            return Err("block table overlaps header");
        }
        range(b, table, 0xb4)?;
        let mut rooms = Vec::new();
        let mut extents = Vec::new();
        let mut slot = table.checked_add(0xb4).ok_or("slot overflow")?;
        loop {
            let h = word(b, slot)? as usize;
            if h == 0 {
                break;
            }
            range(b, h, 0x48)?;
            let n = word(b, h)? as usize;
            let bytes = n.checked_mul(0xc8).ok_or("room count overflow")?;
            let start = h.checked_sub(bytes).ok_or("room underflow")?;
            if start < 48 {
                return Err("rooms overlap header");
            }
            range(b, start, bytes)?;
            extents.push((start, h));
            for i in 0..n {
                let offset = start + i * 0xc8;
                let w = words(b, offset, 50)?;
                for p in POINTERS {
                    if w[p / 4] != 0 {
                        range(b, w[p / 4] as usize, 1)?;
                    }
                }
                rooms.push(Room { offset, words: w });
            }
            slot = slot.checked_add(0xb4).ok_or("slot overflow")?;
        }
        extents.sort();
        for pair in extents.windows(2) {
            if pair[0].1 > pair[1].0 {
                return Err("overlapping room tables");
            }
        }
        let room_offsets = rooms.iter().map(|room| room.offset).collect();
        Ok(Self {
            header,
            rooms,
            original: b.to_vec(),
            room_offsets,
        })
    }
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.header.len() != 12 || self.rooms.iter().any(|r| r.words.len() != 50) {
            return Err("header/room width");
        }
        for room in &self.rooms {
            range(&self.original, room.offset, 200)?;
        }
        if self
            .rooms
            .iter()
            .map(|room| room.offset)
            .ne(self.room_offsets.iter().copied())
        {
            return Err("room offset");
        }
        let mut b = self.original.clone();
        for (i, w) in self.header.iter().enumerate() {
            try_put(&mut b, i * 4, *w)?;
        }
        for r in &self.rooms {
            range(&b, r.offset, 200)?;
            for (i, w) in r.words.iter().enumerate() {
                let offset = r.offset.checked_add(i * 4).ok_or("extent overflow")?;
                try_put(&mut b, offset, *w)?;
            }
        }
        Self::parse(&b)?;
        Ok(b)
    }
    pub fn relocated(&self, base: u32) -> Result<Vec<Vec<u32>>> {
        self.rooms
            .iter()
            .map(|r| {
                if r.words.len() != 50 {
                    return Err("header/room width");
                }
                let mut w = r.words.clone();
                for p in POINTERS {
                    if w[p / 4] != 0 {
                        w[p / 4] = w[p / 4].checked_add(base).ok_or("pointer overflow")?;
                    }
                }
                Ok(w)
            })
            .collect()
    }
}
#[cfg(test)]
fn single_room_envelope() -> Vec<u8> {
    let mut bytes = vec![0; 1024];
    put(&mut bytes, 4, 48);
    put(&mut bytes, 48 + 180, 800);
    put(&mut bytes, 800, 1);
    bytes
}

#[test]
fn room_witness() {
    let mut b = single_room_envelope();
    put(&mut b, 600, 900);
    let l = Level::parse(&b).unwrap();
    assert_eq!(l.rooms.len(), 1);
    assert_eq!(l.encode().unwrap(), b);
    assert_eq!(l.relocated(100).unwrap()[0][0], 1000);
    assert!(l.relocated(u32::MAX).is_err());
    put(&mut b, 800, u32::MAX);
    assert!(Level::parse(&b).is_err());
}

#[test]
fn room_pointer_grid_preserves_envelope_and_rebases_only_nonzero_fields() {
    for seed in 0..256u32 {
        let mut bytes = single_room_envelope();
        for (index, byte) in bytes[900..].iter_mut().enumerate() {
            *byte = seed.wrapping_mul(17).wrapping_add(index as u32) as u8;
        }
        let pointer_index = seed as usize % POINTERS.len();
        let pointer = 900 + seed % 124;
        put(&mut bytes, 600 + POINTERS[pointer_index], pointer);
        let level = Level::parse(&bytes).unwrap();
        assert_eq!(level.rooms.len(), 1);
        assert_eq!(level.encode().unwrap(), bytes);
        let base = 0x1000 + seed;
        let relocated = level.relocated(base).unwrap();
        for (index, offset) in POINTERS.iter().enumerate() {
            let expected = if index == pointer_index {
                pointer + base
            } else {
                0
            };
            assert_eq!(relocated[0][offset / 4], expected);
        }
        assert_eq!(level.encode().unwrap(), bytes);
    }
}

#[test]
fn room_pointer_bounds_and_overflow_grid() {
    for &offset in POINTERS {
        let mut bytes = single_room_envelope();
        put(&mut bytes, 600 + offset, 1024);
        assert!(Level::parse(&bytes).is_err());
        put(&mut bytes, 600 + offset, 1023);
        let level = Level::parse(&bytes).unwrap();
        assert!(level.relocated(u32::MAX).is_err());
    }
}

#[test]
fn edited_room_offset_rejects_overflow_without_panicking() {
    let bytes = single_room_envelope();
    let mut level = Level::parse(&bytes).unwrap();
    level.rooms[0].offset = usize::MAX;
    assert_eq!(level.encode(), Err("extent overflow"));
}

#[test]
fn edited_room_offset_cannot_relocate_into_opaque_bytes() {
    let bytes = single_room_envelope();
    let mut level = Level::parse(&bytes).unwrap();
    level.rooms[0].offset = 601;
    assert_eq!(level.encode(), Err("room offset"));
    assert_eq!(Level::parse(&bytes).unwrap().encode().unwrap(), bytes);
}

#[test]
fn relocated_rejects_edited_room_width_without_panicking() {
    let bytes = single_room_envelope();
    let mut level = Level::parse(&bytes).unwrap();
    let original = level.encode().unwrap();

    level.rooms[0].words.truncate(1);
    assert_eq!(level.relocated(0), Err("header/room width"));
    level.rooms[0].words.clear();
    assert_eq!(level.relocated(0), Err("header/room width"));
    level.rooms[0].words.resize(51, 0);
    assert_eq!(level.relocated(0), Err("header/room width"));
    level.rooms[0].words.truncate(50);
    assert_eq!(level.relocated(0).unwrap()[0].len(), 50);
    assert_eq!(level.encode().unwrap(), original);
}
