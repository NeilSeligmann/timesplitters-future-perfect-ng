//! T1868: the particle template definitions (`0x78DBA0`, `0x78ED40`, `0x7B0A80` and the other effect templates) that
//! `0x111AE0` copies from, built at run time by the original and rebuilt here from a caller supplied XBE image.
//!
//! Where they come from (MEASURED, `docs/t-story-event-effect-pool-t1868.md`): the templates live in BSS (zero in the file). The
//! original fills them by straight line definer functions made only of `push`, `call`, `lea`, `movss` and `add esp`:
//!
//! * `0xDD1B0` (called once from `0xE8E94`, the boot effect table): about 60 templates including `0x7B0A80` (the scratch
//!   template every kind handler pushes) and the ember pair `0x735FC8` / `0x735DE8`,
//! * `0x207740` (called from the level 0x11 init `0x207F0D`): the template `0x78DBA0` (spark / table entry particle),
//! * `0x1F1260` (called from `0x1F650B`, the barrier level init): the template `0x78ED40` and the other barrier templates;
//!   it also spawns emitters (`0x111AE0`, `0x10E1C0`, `0x10E6F0`), which are recorded and skipped here.
//!
//! The definers call three families of setters that write into a "current record" pointer kept in guest memory:
//! family A (`0x1113E0` begin, `0x111450` copy, `0x111480 .. 0x1115D0`) writes a 0x84 byte sprite record through `[0x73BD64]`,
//! family B (`0x1115F0` begin, `0x111660` copy, `0x111680 .. 0x111980`) a 0xCC byte particle spec through `[0x73BD68]`,
//! family C (`0x111990`, `0x111A10`) a 0x38 byte emitter record through `[0x73BD6C]`. All setters are ported here
//! ([`apply_setter`]) and the definer code is executed by a tiny interpreter over the image bytes ([`run_definer`]), so no game
//! byte is committed: the constants come from the image the caller supplies (tests skip without it).
//!
//! Validation: `tools/t1846_gold/svc_template_defs.py` runs the original definers under the Unicorn oracle and writes a hash of
//! the template region plus the target records; `tests/story_events_t1868_templates.rs` compares the interpreter bit for bit.

use super::svc_effect_submit::SvcMemory;
use super::svc_kind_table::bytes_at;

/// Current record pointers (guest globals) of the three setter families.
pub const CURRENT_SPRITE: u32 = 0x0073_BD64;
pub const CURRENT_SPEC: u32 = 0x0073_BD68;
pub const CURRENT_EMITTER: u32 = 0x0073_BD6C;
/// Sizes in dwords: sprite record (`rep stosd 0x21`), particle spec (`0x33`), emitter record (`0xE`).
pub const SPRITE_WORDS: u32 = 0x21;
pub const SPEC_WORDS: u32 = 0x33;
pub const EMITTER_WORDS: u32 = 0xE;
/// The definer functions (entry VA) and the templates they are known to define.
pub const BOOT_EFFECTS: u32 = 0x000D_D1B0;
pub const LEVEL11_TABLE: u32 = 0x0020_7740;
pub const BARRIER_INIT: u32 = 0x001F_1260;
/// The templates this task needs, with the definer that creates them.
pub const TARGETS: [(u32, u32); 3] = [
    (0x0078_DBA0, LEVEL11_TABLE),
    (0x0078_ED40, BARRIER_INIT),
    (0x007B_0A80, BOOT_EFFECTS),
];
/// Where the interpreter keeps its stack inside the guest memory (grows down from the top).
pub const STACK_TOP: u32 = 0x5BC1_0000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DefError {
    /// An instruction the interpreter does not model (address, first bytes).
    Unsupported(u32, [u8; 4]),
    /// The definer or a constant lies outside the image.
    OutsideImage(u32),
    /// The step budget ran out.
    Runaway,
}

/// What a definer run did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DefReport {
    /// Instructions executed.
    pub steps: usize,
    /// Setter calls executed.
    pub setters: usize,
    /// Calls that were recorded and skipped (`(target, first five stack words)`).
    pub skipped: Vec<(u32, [u32; 5])>,
}

fn f(bits: u32) -> f32 {
    f32::from_bits(bits)
}

/// The setters of the three families. `args` are the cdecl stack words (`args[0]` is the first argument).
/// Returns false when `target` is not a setter.
pub fn apply_setter(mem: &mut dyn SvcMemory, target: u32, args: &dyn Fn(u32) -> u32) -> bool {
    let a = |n: u32| args(n);
    let sprite = mem.r32(CURRENT_SPRITE);
    let spec = mem.r32(CURRENT_SPEC);
    match target {
        // ---- family A: 0x84 byte sprite records through [0x73BD64]
        0x1113E0 => {
            let record = a(0);
            for k in 0..SPRITE_WORDS {
                mem.w32(record + 4 * k, 0);
            }
            mem.w32(record + 0x20, a(4));
            mem.w32(record + 0x24, a(5));
            mem.w32(record + 0x4C, a(1));
            mem.w32(record + 0x30, a(6));
            mem.w32(CURRENT_SPRITE, record);
            mem.w32(record, a(2));
            mem.w32(record + 4, a(3));
            mem.w32(record + 0x54, u32::MAX);
            for offset in [0x58, 0x5C, 0x60] {
                mem.w32(record + offset, 1.0f32.to_bits());
            }
        }
        0x111450 => {
            let (record, source) = (a(0), a(2));
            for k in 0..SPRITE_WORDS {
                let word = mem.r32(source + 4 * k);
                mem.w32(record + 4 * k, word);
            }
            mem.w32(CURRENT_SPRITE, record);
            mem.w32(record + 0x4C, a(1));
        }
        0x111480 => {
            mem.w32(sprite + 0x20, a(0));
            mem.w32(sprite + 0x24, a(1));
            mem.w32(sprite + 0x30, a(2));
        }
        0x1114B0 => {
            let pointer = a(0);
            for k in 0..3 {
                let word = mem.r32(pointer + 4 * k);
                mem.w32(sprite + 0x58 + 4 * k, word);
            }
        }
        0x1114D0 => {
            mem.w32(sprite + 0x28, a(0));
            mem.w32(sprite + 0x2C, a(1));
        }
        0x1114F0 => {
            mem.w32(sprite + 0x34, a(0));
            mem.w32(sprite + 0x38, a(1));
        }
        0x111510 => {
            for k in 0..4 {
                mem.w32(sprite + 0x3C + 4 * k, a(k));
            }
        }
        0x111550 => mem.w32(sprite + 0x50, a(0)),
        0x111560 => {
            if sprite != 0 {
                mem.w32(sprite + 0x70, a(0));
            }
        }
        0x111580 => {
            if sprite != 0 {
                mem.w32(sprite + 0x74, a(0));
                mem.w32(sprite + 0x78, a(1));
            }
        }
        0x1115A0 => {
            if sprite != 0 {
                mem.w32(sprite + 0x7C, a(0));
                mem.w32(sprite + 0x80, a(1));
            }
        }
        0x1115D0 => {
            if sprite != 0 {
                mem.w32(sprite + 0x54, a(0));
            }
        }
        // ---- family B: 0xCC byte particle specs through [0x73BD68]
        0x1115F0 => {
            let record = a(0);
            for k in 0..SPEC_WORDS {
                mem.w32(record + 4 * k, 0);
            }
            mem.w32(record + 0x14, a(2));
            mem.w32(record, a(3));
            mem.w32(record + 0x10, a(5));
            mem.w32(record + 4, a(4));
            mem.w32(CURRENT_SPEC, record);
            mem.w32(record + 0xC, a(1));
            mem.w32(record + 0x18, a(6));
            mem.w32(record + 0x1C, a(7));
            if a(1) == 5 || a(1) == 6 {
                mem.w32(record + 0xA0, 1.0f32.to_bits());
            }
        }
        0x111660 => {
            let (record, source) = (a(0), a(1));
            mem.w32(CURRENT_SPEC, record);
            for k in 0..SPEC_WORDS {
                let word = mem.r32(source + 4 * k);
                mem.w32(record + 4 * k, word);
            }
        }
        0x111680 => {
            let (first, second) = (f(a(4)), f(a(5)));
            let mut count = 0u32;
            if first > 0.0 || second > 0.0 {
                mem.w32(spec + 0x38, a(4));
                mem.w32(spec + 0x3C, 0.0f32.to_bits());
                count = 1;
            }
            let put = |mem: &mut dyn SvcMemory, count: u32, low: u32, high: u32| {
                mem.w32(spec + count * 8 + 0x38, low);
                mem.w32(spec + count * 8 + 0x3C, high);
            };
            put(mem, count, a(5), a(3));
            count += 1;
            if 1.0f32 > f(a(6)) {
                put(mem, count, a(6), a(3));
                count += 1;
            }
            put(mem, count, 1.0f32.to_bits(), 0.0f32.to_bits());
            count += 1;
            mem.w32(spec + 0x34, count);
            mem.w32(spec + 0x20, a(0));
            mem.w32(spec + 0x2C, a(1));
            mem.w32(spec + 0x30, a(2));
        }
        0x111710 => {
            let (first, second, count) = (a(0), a(1), a(2));
            for k in 0..count {
                let (low, high) = (mem.r32(first + 4 * k), mem.r32(second + 4 * k));
                mem.w32(spec + 0x38 + 8 * k, low);
                mem.w32(spec + 0x3C + 8 * k, high);
            }
            mem.w32(spec + 0x34, count);
        }
        0x1117D0 => {
            mem.w32(spec + 0x24, a(0));
            mem.w32(spec + 0x28, a(1));
        }
        0x1117F0 => {
            for (offset, n) in [
                (0x60, 0),
                (0x68, 2),
                (0x64, 1),
                (0x6C, 3),
                (0x70, 4),
                (0x74, 5),
            ] {
                mem.w32(spec + offset, a(n));
            }
        }
        0x111840 => {
            mem.w32(spec + 0x78, a(0));
            mem.w32(spec + 0x70, a(1));
        }
        0x111860 => {
            for k in 0..4 {
                mem.w32(spec + 0x78 + 4 * k, a(k));
            }
        }
        0x1118A0 => {
            for group in 0..4u32 {
                let pointer = a(group);
                for k in 0..3 {
                    let word = mem.r32(pointer + 4 * k);
                    mem.w32(spec + 0x9C + 12 * group + 4 * k, word);
                }
            }
        }
        0x111920 => {
            for k in 0..4 {
                mem.w32(spec + 0x88 + 4 * k, a(k));
            }
        }
        0x111960 => mem.w32(spec + 0x98, a(0)),
        0x111980 => mem.w32(spec + 0x10, a(0)),
        // ---- family C: 0x38 byte emitter records through [0x73BD6C]
        0x111990 => {
            let record = a(0);
            for k in 0..EMITTER_WORDS {
                mem.w32(record + 4 * k, 0);
            }
            let pointer = a(1);
            for k in 0..2 {
                let word = mem.r32(pointer + 4 * k);
                mem.w32(record + 4 * k, word);
            }
            let third = mem.r32(pointer + 8);
            mem.w32(record + 0xC, a(2));
            mem.w32(record + 0x10, (a(3) as i32 as f32).to_bits());
            mem.w32(record + 0x1C, a(6));
            mem.w32(record + 0x20, a(7));
            mem.w32(record + 8, third);
            mem.w32(record + 0x24, a(8));
            mem.w32(CURRENT_EMITTER, record);
            mem.w32(record + 0x14, a(4));
            mem.w32(record + 0x18, a(5));
            mem.w32(record + 0x28, a(9));
        }
        0x111A10 => {
            let emitter = mem.r32(CURRENT_EMITTER);
            mem.w32(emitter + 0x2C, a(0));
            mem.w32(emitter + 0x30, a(1));
            mem.w32(emitter + 0x34, a(2));
        }
        _ => return false,
    }
    true
}

/// Machine state of the interpreter.
struct Machine<'a> {
    image: &'a [u8],
    mem: &'a mut dyn SvcMemory,
    regs: [u32; 8],
    xmm: [u32; 8],
    esp: u32,
    report: DefReport,
}

const EAX: usize = 0;
const ECX: usize = 1;
const EDX: usize = 2;
const ESP: usize = 4;

impl Machine<'_> {
    fn byte(&self, address: u32) -> Result<u8, DefError> {
        bytes_at(self.image, address, 1)
            .map(|b| b[0])
            .ok_or(DefError::OutsideImage(address))
    }
    fn word(&self, address: u32) -> Result<u32, DefError> {
        let bytes = bytes_at(self.image, address, 4).ok_or(DefError::OutsideImage(address))?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
    fn push(&mut self, value: u32) {
        self.esp = self.esp.wrapping_sub(4);
        self.mem.w32(self.esp, value);
    }
    /// Effective address of a modrm memory operand at `at` (after the modrm byte). Returns (address, bytes consumed after modrm).
    fn address(&self, modrm: u8, at: u32) -> Result<(u32, u32), DefError> {
        let (mode, rm) = (modrm >> 6, modrm & 7);
        let unsupported = || DefError::Unsupported(at, [modrm, 0, 0, 0]);
        match (mode, rm) {
            (0, 5) => Ok((self.word(at)?, 4)),
            (0 | 1 | 2, 4) => {
                if self.byte(at)? != 0x24 {
                    return Err(unsupported());
                }
                match mode {
                    0 => Ok((self.esp, 1)),
                    1 => Ok((
                        self.esp
                            .wrapping_add(self.byte(at + 1)? as i8 as i32 as u32),
                        2,
                    )),
                    _ => Ok((self.esp.wrapping_add(self.word(at + 1)?), 5)),
                }
            }
            _ => Err(unsupported()),
        }
    }
    fn read_operand(&self, address: u32) -> u32 {
        // absolute addresses inside the image are constants; everything else is guest memory
        match bytes_at(self.image, address, 4) {
            Some(b) if address < 0x0055_0000 => u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            _ => self.mem.r32(address),
        }
    }

    /// Run one function body from `pc` until its `ret` (or until `stop`, when given). `frame_depth` > 0 for nested calls.
    fn run(&mut self, mut pc: u32, stop: Option<u32>, depth: u32) -> Result<(), DefError> {
        loop {
            if stop == Some(pc) {
                return Ok(());
            }
            self.report.steps += 1;
            if self.report.steps > 400_000 {
                return Err(DefError::Runaway);
            }
            let op = self.byte(pc)?;
            let bad = |this: &Self| -> DefError {
                let b = |k| this.byte(pc + k).unwrap_or(0);
                DefError::Unsupported(pc, [b(0), b(1), b(2), b(3)])
            };
            match op {
                0x68 => {
                    let value = self.word(pc + 1)?;
                    self.push(value);
                    pc += 5;
                }
                0x6A => {
                    let value = self.byte(pc + 1)? as i8 as i32 as u32;
                    self.push(value);
                    pc += 2;
                }
                0x50..=0x57 => {
                    let value = self.regs[(op - 0x50) as usize];
                    self.push(value);
                    pc += 1;
                }
                0xE8 => {
                    let target = pc.wrapping_add(5).wrapping_add(self.word(pc + 1)?);
                    let esp = self.esp;
                    let arg = |this: &Self, n: u32| this.mem.r32(esp.wrapping_add(4 * n));
                    let args: Vec<u32> = (0..12).map(|n| arg(self, n)).collect();
                    if apply_setter(self.mem, target, &|n| args[n as usize]) {
                        self.report.setters += 1;
                    } else if matches!(target, 0x1F1040 | 0x1F1180) {
                        self.push(0);
                        self.run(target, None, depth + 1)?;
                    } else if matches!(target, 0x10E1C0 | 0x10E6F0 | 0x111AE0) {
                        self.report
                            .skipped
                            .push((target, [args[0], args[1], args[2], args[3], args[4]]));
                    } else {
                        return Err(bad(self));
                    }
                    pc += 5;
                }
                0x83 | 0x81 if matches!(self.byte(pc + 1)?, 0xC4 | 0xEC) => {
                    let (value, size) = if op == 0x83 {
                        (self.byte(pc + 2)? as i8 as i32 as u32, 3)
                    } else {
                        (self.word(pc + 2)?, 6)
                    };
                    self.esp = if self.byte(pc + 1)? == 0xC4 {
                        self.esp.wrapping_add(value)
                    } else {
                        self.esp.wrapping_sub(value)
                    };
                    pc += size;
                }
                0x8D => {
                    let modrm = self.byte(pc + 1)?;
                    let (address, used) = self.address(modrm, pc + 2)?;
                    self.regs[((modrm >> 3) & 7) as usize] = address;
                    pc += 2 + used;
                }
                0x8B | 0x89 => {
                    let modrm = self.byte(pc + 1)?;
                    if modrm >> 6 == 3 {
                        let (reg, rm) = (((modrm >> 3) & 7) as usize, (modrm & 7) as usize);
                        if op == 0x8B {
                            self.regs[reg] = self.regs[rm];
                        } else {
                            self.regs[rm] = self.regs[reg];
                        }
                        pc += 2;
                    } else if op == 0x8B {
                        let (address, used) = self.address(modrm, pc + 2)?;
                        self.regs[((modrm >> 3) & 7) as usize] = self.read_operand(address);
                        pc += 2 + used;
                    } else {
                        return Err(bad(self));
                    }
                }
                0xA1 => {
                    let address = self.word(pc + 1)?;
                    self.regs[EAX] = self.read_operand(address);
                    pc += 5;
                }
                0x0F if self.byte(pc + 1)? == 0x57 && self.byte(pc + 2)? == 0xC0 => {
                    self.xmm[0] = 0;
                    pc += 3;
                }
                0xF3 if self.byte(pc + 1)? == 0x0F && matches!(self.byte(pc + 2)?, 0x10 | 0x11) => {
                    let store = self.byte(pc + 2)? == 0x11;
                    let modrm = self.byte(pc + 3)?;
                    let xmm = ((modrm >> 3) & 7) as usize;
                    let (address, used) = self.address(modrm, pc + 4)?;
                    if store {
                        self.mem.w32(address, self.xmm[xmm]);
                    } else {
                        self.xmm[xmm] = self.read_operand(address);
                    }
                    pc += 4 + used;
                }
                0xC3 => {
                    if depth > 0 {
                        self.esp = self.esp.wrapping_add(4);
                    }
                    return Ok(());
                }
                _ => return Err(bad(self)),
            }
        }
    }
}

/// Run the definer function at `entry` of `image` over `mem` (the stack lives at [`STACK_TOP`] in the same memory) up to its
/// `ret`. `stop` ends the run early at that address (a segment of a longer function).
pub fn run_definer(
    image: &[u8],
    mem: &mut dyn SvcMemory,
    entry: u32,
    stop: Option<u32>,
) -> Result<DefReport, DefError> {
    let mut machine = Machine {
        image,
        mem,
        regs: [0; 8],
        xmm: [0; 8],
        esp: STACK_TOP,
        report: DefReport::default(),
    };
    machine.regs[ESP] = STACK_TOP;
    let _ = (ECX, EDX);
    machine.run(entry, stop, 0)?;
    Ok(machine.report)
}

/// Define every template of [`TARGETS`] (each definer once) in `mem` from `image`.
pub fn load_templates(
    image: &[u8],
    mem: &mut dyn SvcMemory,
) -> Result<Vec<(u32, DefReport)>, DefError> {
    let mut reports = Vec::new();
    for entry in [BOOT_EFFECTS, LEVEL11_TABLE, BARRIER_INIT] {
        reports.push((entry, run_definer(image, mem, entry, None)?));
    }
    Ok(reports)
}
