//! GP DSP DMA engine: walks descriptor nodes held in DSP memory and moves
//! data between DSP memory and the scratch/FIFO buffers.
//!
//! Behaviour follows the pinned xemu reference as exercised by the game's
//! effects image. Unsupported descriptors are reported as errors instead of
//! asserting, matching the C adapter's guarded domain (buffers 14/15 only,
//! 8/16/24-bit formats, no unknown control bits).

pub const CONFIGURATION_AUTOSTART: u32 = 1;
pub const CONTROL_ACTION: u32 = 7;
pub const ACTION_START: u32 = 1;
pub const ACTION_STOP: u32 = 2;
pub const ACTION_FREEZE: u32 = 3;
pub const ACTION_UNFREEZE: u32 = 4;
pub const CONTROL_FROZEN: u32 = 1 << 3;
pub const CONTROL_RUNNING: u32 = 1 << 4;
pub const CONTROL_STOPPED: u32 = 1 << 5;
const NODE_POINTER_VAL: u32 = 0x3fff;
const NODE_POINTER_EOL: u32 = 1 << 14;
const MAX_NODES: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Space {
    X,
    Y,
    P,
}

/// What the DMA engine needs from its surroundings.
pub trait DmaHost {
    fn mem_read(&mut self, space: Space, address: u32) -> u32;
    fn mem_write(&mut self, space: Space, address: u32, value: u32);
    /// Read or write `bytes` at a scratch address; false if out of range.
    fn scratch(&mut self, bytes: &mut [u8], address: usize, write: bool) -> bool;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DmaError {
    UnsupportedAction,
    UnsupportedDescriptor,
    ScratchRange,
}

#[derive(Default)]
pub struct Dma {
    pub configuration: u32,
    pub control: u32,
    pub start_block: u32,
    pub next_block: u32,
    pub eol: bool,
    read_count: u32,
}

/// Map a DMA address to a DSP memory space and word address.
fn locate(address: u32, words: u32) -> Option<(Space, u32)> {
    let (space, base, end) = match address {
        0..0x1800 => (Space::X, address, 0x1800),
        0x1800..0x2000 => (Space::Y, address - 0x1800, 0x2000),
        0x2800..0x3800 => (Space::P, address - 0x2800, 0x3800),
        _ => return None,
    };
    (address + words < end).then_some((space, base))
}

struct Node {
    next: u32,
    control: u32,
    count: u32,
    dsp_offset: u32,
    scratch_offset: u32,
    scratch_base: u32,
    scratch_size: u32,
}

impl Node {
    fn read(host: &mut impl DmaHost, space: Space, base: u32) -> Node {
        let mut word = |index| host.mem_read(space, base + index);
        Node {
            next: word(0),
            control: word(1),
            count: word(2),
            dsp_offset: word(3),
            scratch_offset: word(4),
            scratch_base: word(5),
            scratch_size: word(6).wrapping_add(1),
        }
    }
    fn to_scratch(&self) -> bool {
        self.control & 2 != 0
    }
    fn interleave(&self) -> bool {
        self.control & 1 != 0
    }
    fn writeback(&self) -> bool {
        self.control & 0x10 != 0
    }
    fn buffer(&self) -> u32 {
        (self.control >> 5) & 15
    }
    fn format(&self) -> u32 {
        (self.control >> 10) & 7
    }
    /// Bytes per item and the mask applied when reading into DSP memory.
    fn item(&self) -> Option<(usize, u32)> {
        match self.format() {
            0 => Some((1, 0xff)),
            1 => Some((2, 0xffff)),
            2 | 6 => Some((4, 0xff_ffff)),
            _ => None,
        }
    }
}

impl Dma {
    pub fn read(&mut self, register: Register) -> u32 {
        match register {
            Register::Configuration => self.configuration,
            Register::Control => {
                if self.control & CONTROL_RUNNING != 0 {
                    self.read_count += 1;
                    if self.read_count > 2 {
                        self.control &= !CONTROL_RUNNING;
                        self.control |= CONTROL_STOPPED;
                        self.read_count = 0;
                    }
                }
                self.control
            }
            Register::StartBlock => self.start_block,
            Register::NextBlock => self.next_block,
        }
    }

    pub fn write(
        &mut self,
        host: &mut impl DmaHost,
        register: Register,
        value: u32,
    ) -> Result<(), DmaError> {
        match register {
            Register::Configuration => self.configuration = value,
            Register::StartBlock => self.start_block = value,
            Register::NextBlock => self.next_block = value,
            Register::Control => return self.write_control(host, value),
        }
        Ok(())
    }

    fn write_control(&mut self, host: &mut impl DmaHost, value: u32) -> Result<(), DmaError> {
        match value & CONTROL_ACTION {
            ACTION_START => {
                self.control |= CONTROL_RUNNING;
                self.control &= !CONTROL_STOPPED;
                self.read_count = 0;
            }
            ACTION_STOP => {
                self.control |= CONTROL_STOPPED;
                self.control &= !CONTROL_RUNNING;
            }
            ACTION_FREEZE => self.control |= CONTROL_FROZEN,
            ACTION_UNFREEZE => self.control &= !CONTROL_FROZEN,
            _ => return Err(DmaError::UnsupportedAction),
        }
        self.run(host)
    }

    /// Check the chain from `next_block` stays inside the supported domain.
    fn chain_supported(&self, host: &mut impl DmaHost) -> bool {
        let mut next = self.next_block;
        for _ in 0..MAX_NODES {
            if next & NODE_POINTER_EOL != 0 {
                return true;
            }
            let Some((space, base)) = locate(next & NODE_POINTER_VAL, 6) else {
                return false;
            };
            let node = Node::read(host, space, base);
            let supported_format = matches!(node.format(), 0 | 1 | 2 | 6);
            let supported_buffer = matches!(node.buffer(), 14 | 15);
            let bad_interleave = node.interleave() && (!node.to_scratch() || node.format() == 0);
            if node.control & 0x200c != 0
                || !supported_format
                || !supported_buffer
                || bad_interleave
                || locate(node.dsp_offset, node.count).is_none()
            {
                return false;
            }
            next = node.next;
        }
        false
    }

    fn run(&mut self, host: &mut impl DmaHost) -> Result<(), DmaError> {
        if self.control & CONTROL_RUNNING == 0 || self.control & CONTROL_FROZEN != 0 {
            return Ok(());
        }
        if !self.chain_supported(host) {
            return Err(DmaError::UnsupportedDescriptor);
        }
        while self.next_block & NODE_POINTER_EOL == 0 {
            let (space, base) = locate(self.next_block & NODE_POINTER_VAL, 6)
                .ok_or(DmaError::UnsupportedDescriptor)?;
            let node = Node::read(host, space, base);
            self.next_block = node.next;
            if node.next & NODE_POINTER_EOL != 0 {
                self.eol = true;
            }
            self.transfer(host, &node)?;
            if node.writeback() {
                let offset = self.scratch_offset_after(&node);
                host.mem_write(space, base + 4, offset);
            }
        }
        Ok(())
    }

    /// Scratch offset after a node ran (only the circular buffer advances it).
    fn scratch_offset_after(&self, node: &Node) -> u32 {
        if node.buffer() != 14 {
            return node.scratch_offset;
        }
        let (item, _) = node.item().unwrap_or((4, 0));
        let bytes = if node.to_scratch() && node.interleave() {
            (node.count >> 4) * item as u32 * ((node.count & 15) + 1)
        } else {
            node.count * item as u32
        };
        circular_end(node.scratch_offset, node.scratch_size, bytes)
    }

    fn transfer(&self, host: &mut impl DmaHost, node: &Node) -> Result<(), DmaError> {
        let (item, mask) = node.item().ok_or(DmaError::UnsupportedDescriptor)?;
        let (space, address) =
            locate(node.dsp_offset, node.count).ok_or(DmaError::UnsupportedDescriptor)?;
        let count = node.count as usize;
        let mut buffer = vec![0u8; count * item];
        if node.to_scratch() {
            let (channels, blocks) = ((node.count & 15) as usize + 1, (node.count >> 4) as usize);
            if node.interleave() {
                buffer.truncate(blocks * item * channels);
                for i in 0..blocks {
                    for ch in 0..channels {
                        let word = host.mem_read(space, address + (ch * blocks + i) as u32);
                        put_item(&mut buffer[(i * channels + ch) * item..], item, word);
                    }
                }
            } else {
                for i in 0..count {
                    let word = host.mem_read(space, address + i as u32);
                    put_item(&mut buffer[i * item..], item, word);
                }
            }
            self.scratch_io(host, node, &mut buffer, true)
        } else {
            self.scratch_io(host, node, &mut buffer, false)?;
            for i in 0..count {
                let raw = &buffer[i * item..];
                let word = match item {
                    2 => u32::from(u16::from_le_bytes([raw[0], raw[1]])) << 8,
                    1 => u32::from(raw[0]) & mask,
                    _ => u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]) & mask,
                };
                host.mem_write(space, address + i as u32, word);
            }
            Ok(())
        }
    }

    fn scratch_io(
        &self,
        host: &mut impl DmaHost,
        node: &Node,
        buffer: &mut [u8],
        write: bool,
    ) -> Result<(), DmaError> {
        let ok = match node.buffer() {
            15 => {
                let address = node.scratch_base as usize + node.scratch_offset as usize;
                host.scratch(buffer, address, write)
            }
            14 => circular_copy(host, node, buffer, write),
            _ => return Err(DmaError::UnsupportedDescriptor),
        };
        ok.then_some(()).ok_or(DmaError::ScratchRange)
    }
}

/// Offset after moving `bytes` through a circular scratch window.
fn circular_end(offset: u32, size: u32, bytes: u32) -> u32 {
    if size == 0 {
        return 0;
    }
    let start = if offset >= size { 0 } else { offset };
    ((u64::from(start) + u64::from(bytes)) % u64::from(size)) as u32
}

fn circular_copy(host: &mut impl DmaHost, node: &Node, buffer: &mut [u8], write: bool) -> bool {
    let size = node.scratch_size as usize;
    let mut offset = if node.scratch_offset >= node.scratch_size {
        0
    } else {
        node.scratch_offset as usize
    };
    let mut done = 0;
    while done < buffer.len() {
        let chunk = (buffer.len() - done).min(size - offset);
        let address = node.scratch_base as usize + offset;
        if !host.scratch(&mut buffer[done..done + chunk], address, write) {
            return false;
        }
        offset = (offset + chunk) % size;
        done += chunk;
    }
    true
}

fn put_item(out: &mut [u8], item: usize, word: u32) {
    match item {
        1 => out[0] = word as u8,
        2 => out[..2].copy_from_slice(&((word >> 8) as u16).to_le_bytes()),
        _ => out[..4].copy_from_slice(&word.to_le_bytes()),
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Register {
    Configuration,
    Control,
    StartBlock,
    NextBlock,
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake {
        x: Vec<u32>,
        scratch: Vec<u8>,
    }
    impl DmaHost for Fake {
        fn mem_read(&mut self, space: Space, address: u32) -> u32 {
            assert_eq!(space, Space::X);
            self.x[address as usize]
        }
        fn mem_write(&mut self, space: Space, address: u32, value: u32) {
            assert_eq!(space, Space::X);
            self.x[address as usize] = value;
        }
        fn scratch(&mut self, bytes: &mut [u8], address: usize, write: bool) -> bool {
            let Some(w) = self.scratch.get_mut(address..address + bytes.len()) else {
                return false;
            };
            if write {
                w.copy_from_slice(bytes);
            } else {
                bytes.copy_from_slice(w);
            }
            true
        }
    }

    fn node(host: &mut Fake, at: usize, words: [u32; 7]) {
        host.x[at..at + 7].copy_from_slice(&words);
    }

    fn start(dma: &mut Dma, host: &mut Fake, first: u32) -> Result<(), DmaError> {
        dma.write(host, Register::NextBlock, first)?;
        dma.write(host, Register::Control, ACTION_START)
    }

    #[test]
    fn scratch_to_dsp_and_back_24_bit() {
        let mut host = Fake {
            x: vec![0; 0x100],
            scratch: vec![0; 0x100],
        };
        host.scratch[16..24].copy_from_slice(&[1, 2, 3, 0xff, 4, 5, 6, 0xff]);
        // buffer 15, format 2, from scratch into X[0x40..0x42]
        node(
            &mut host,
            0,
            [NODE_POINTER_EOL, 15 << 5 | 2 << 10, 2, 0x40, 16, 0, 0xff],
        );
        let mut dma = Dma::default();
        start(&mut dma, &mut host, 0).unwrap();
        assert_eq!(host.x[0x40..0x42], [0x030201, 0x060504]);
        assert!(dma.eol);
        // and back to scratch at 64
        node(
            &mut host,
            0,
            [
                NODE_POINTER_EOL,
                15 << 5 | 2 << 10 | 2,
                2,
                0x40,
                64,
                0,
                0xff,
            ],
        );
        let mut dma = Dma::default();
        start(&mut dma, &mut host, 0).unwrap();
        assert_eq!(host.scratch[64..72], [1, 2, 3, 0, 4, 5, 6, 0]);
    }

    #[test]
    fn circular_wraps_and_writes_offset_back() {
        let mut host = Fake {
            x: vec![0; 0x100],
            scratch: vec![0; 0x100],
        };
        for i in 0..4 {
            host.x[0x40 + i] = 0x0a0b00 + i as u32;
        }
        // 4 words * 4 bytes through a 12-byte window starting at offset 8
        node(
            &mut host,
            0,
            [
                NODE_POINTER_EOL,
                14 << 5 | 2 << 10 | 2 | 0x10,
                4,
                0x40,
                8,
                32,
                11,
            ],
        );
        let mut dma = Dma::default();
        start(&mut dma, &mut host, 0).unwrap();
        // word 3 wraps onto the slot word 0 first used
        assert_eq!(host.scratch[40..44], [3, 0x0b, 0x0a, 0]);
        assert_eq!(host.scratch[32..36], [1, 0x0b, 0x0a, 0]);
        assert_eq!(host.x[4], (8 + 16) % 12);
    }

    #[test]
    fn interleave_and_unsupported_domains() {
        let mut host = Fake {
            x: vec![0; 0x100],
            scratch: vec![0; 0x100],
        };
        host.x[0x40..0x44].copy_from_slice(&[1, 2, 3, 4]); // 2 channels, 2 blocks
        node(
            &mut host,
            0,
            [
                NODE_POINTER_EOL,
                15 << 5 | 2 << 10 | 3,
                0x21,
                0x40,
                0,
                0,
                0xff,
            ],
        );
        let mut dma = Dma::default();
        start(&mut dma, &mut host, 0).unwrap();
        let words: Vec<u32> = (0..4).map(|i| word_at(&host.scratch, i)).collect();
        assert_eq!(words, [1, 3, 2, 4]);
        // buffer 0 (FIFO) is outside the supported domain
        node(
            &mut host,
            0,
            [NODE_POINTER_EOL, 2 << 10, 1, 0x40, 0, 0, 0xff],
        );
        let mut dma = Dma::default();
        assert_eq!(
            start(&mut dma, &mut host, 0),
            Err(DmaError::UnsupportedDescriptor)
        );
    }

    fn word_at(bytes: &[u8], i: usize) -> u32 {
        u32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap())
    }

    #[test]
    fn control_register_stops_running_after_three_reads() {
        let mut dma = Dma {
            control: CONTROL_RUNNING,
            ..Dma::default()
        };
        assert_eq!(
            dma.read(Register::Control) & CONTROL_RUNNING,
            CONTROL_RUNNING
        );
        dma.read(Register::Control);
        let v = dma.read(Register::Control);
        assert_eq!(v & CONTROL_RUNNING, 0);
        assert_ne!(v & CONTROL_STOPPED, 0);
    }
}
