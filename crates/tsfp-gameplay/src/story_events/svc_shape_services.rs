//! T1869: the shape engine services below `0x97A00` / `0x965F0` / `0x97C00` (`0x8FE30`, `0x976E0`, `0x8FB20` with
//! `0x8FAA0`, `0x18FDC0`). MEASURED against the Unicorn oracle (`docs/t-story-event-relocators-t1869.md`); names are
//! INFERRED, addresses, offsets and control flow MEASURED. The pose and skeleton engine below `0x976E0` (`0x58270`
//! and the `0x59xxx` / `0x58xxx` family, `0x96860`, `0x1506B0`, `0x1531D0`, `0x153240`) is ported in `svc_pose`
//! (T1872) and routed by [`PortedShapeServices`]; only its deeper callees (`0x58DF0`, `0x1526C0`, `0x903D0`) stay
//! behind [`DestroyEffects`]. Routines with a frame take `entry`, the esp at their entry.

use super::strip_update::GuestMemory;
use super::svc_objects::DestroyEffects;
use super::svc_shape::model_scale;

pub mod callee {
    pub const MATRIX_COPY: u32 = 0x15_31D0; // E cdecl (destination, source), 64 bytes
    pub const POSE_CONTEXT: u32 = 0x5_8270; // E (table word) -> pose context
    pub const POSE_BONE: u32 = 0x5_9320; // E (context, index, 0, out vector, scale)
    pub const QUAT_MATRIX: u32 = 0x15_06B0; // E (matrix, quaternion)
    pub const POSE_A: u32 = 0x5_83E0; // E (slot, 0, 1)
    pub const POSE_B: u32 = 0x5_82A0; // E (slot, matrices, 0, -1)
    pub const POSE_C: u32 = 0x5_8340; // E (slot, matrices, 0, -1)
    pub const POSE_D: u32 = 0x5_9BF0; // E (matrices)
    pub const IDENTITY: u32 = 0x15_3240; // E (matrix)
    pub const POSE_E: u32 = 0x5_9C00; // E (matrix, 1)
    pub const POSE_F: u32 = 0x9_6860; // E (slot, 0, vector)
    pub const POSE_DONE: u32 = 0x5_9CB0; // E ()
}

const SCRATCH_BLOCK: u32 = 0x70_AAD4;

fn r32(memory: &dyn GuestMemory, address: u32) -> u32 {
    memory.read_u32(address)
}

fn w32(memory: &mut dyn GuestMemory, address: u32, value: u32) {
    memory.write_u32(address, value);
}

fn sx8(memory: &dyn GuestMemory, address: u32) -> u32 {
    i32::from(memory.read_u8(address) as i8) as u32
}

/// `0x8FE30` (esi = `slot`, eax = `counter`; stack `matrices`): numbers the parts of the slot's model block. Every
/// part whose byte `+0x48` is 0 gets the next number: its parent byte (`+0x49`) is queued in `[[slot+0x8C]+0][n]`,
/// `[[slot+0x8C]+4][part] = n` and, with `matrices`, the parent's matrix (`[slot+0x10] + 64 * parent`) is copied
/// (`0x1531D0`) to `matrices + 64 * n`; then every part of kind 2 (byte `+0x48`) takes the number of its ancestor
/// `+0x4B` and copies the parent matrix of its parent. MEASURED.
pub fn shape_done(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    slot: u32,
    counter: u32,
    matrices: u32,
) {
    let block = r32(memory, slot.wrapping_add(4));
    let count = r32(memory, block);
    let base = block.wrapping_sub(count.wrapping_mul(0xC8));
    if (r32(memory, block) as i32) > 0 {
        let mut part = base.wrapping_add(0x49);
        let mut index = 0u32;
        loop {
            if memory.read_u8(part.wrapping_sub(1)) == 0 {
                if matrices != 0 {
                    let number = r32(memory, counter);
                    let parent = sx8(memory, part);
                    let source = parent
                        .wrapping_shl(6)
                        .wrapping_add(r32(memory, slot.wrapping_add(0x10)));
                    effects.call(
                        memory,
                        callee::MATRIX_COPY,
                        &[source, number.wrapping_shl(6).wrapping_add(matrices)],
                    );
                }
                let queue = r32(memory, r32(memory, slot.wrapping_add(0x8C)).wrapping_add(4));
                let number = r32(memory, counter);
                w32(memory, queue.wrapping_add(index.wrapping_mul(4)), number);
                let parent = sx8(memory, part);
                let list = r32(memory, r32(memory, slot.wrapping_add(0x8C)));
                let number = r32(memory, counter);
                w32(memory, list.wrapping_add(number.wrapping_mul(4)), parent);
                w32(memory, counter, number.wrapping_add(1));
            }
            index = index.wrapping_add(1);
            part = part.wrapping_add(0xC8);
            if (index as i32) >= (r32(memory, r32(memory, slot.wrapping_add(4))) as i32) {
                break;
            }
        }
    }
    if (r32(memory, r32(memory, slot.wrapping_add(4))) as i32) > 0 {
        let mut part = base.wrapping_add(0x4B);
        let mut index = 0u32;
        loop {
            if memory.read_u8(part.wrapping_sub(3)) == 2 {
                if matrices != 0 {
                    let ancestor = sx8(memory, part).wrapping_mul(0xC8);
                    let grand = sx8(memory, base.wrapping_add(ancestor).wrapping_add(0x49));
                    let parent = sx8(memory, part.wrapping_sub(2));
                    let table = r32(memory, slot.wrapping_add(0x10));
                    effects.call(
                        memory,
                        callee::MATRIX_COPY,
                        &[
                            parent.wrapping_shl(6).wrapping_add(table),
                            grand.wrapping_shl(6).wrapping_add(table),
                        ],
                    );
                }
                let ancestor = sx8(memory, part);
                let queue = r32(memory, r32(memory, slot.wrapping_add(0x8C)).wrapping_add(4));
                let value = r32(memory, queue.wrapping_add(ancestor.wrapping_mul(4)));
                w32(memory, queue.wrapping_add(index.wrapping_mul(4)), value);
            }
            index = index.wrapping_add(1);
            part = part.wrapping_add(0xC8);
            if (index as i32) >= (r32(memory, r32(memory, slot.wrapping_add(4))) as i32) {
                break;
            }
        }
    }
}

/// `0x976E0` (ecx = `slot`, eax = `word`; stack `flag`; frame `0x8C`, `entry` = esp at entry): builds the pose of a
/// dynamic shape slot. The pose context of `word` (`0x58270`) gives the bone count (`+0x14`); every bone is posed
/// (`0x59320` with the model scale of `[slot+4]`), turned into a matrix in the scratch block `[0x70AAD4]`
/// (`0x1506B0`, translation cells `+0x30..+0x38`) and copied into `[[slot+0x8C]+0x78]` (`0x1531D0`). The numbering
/// service `0x8FE30` runs over the scratch matrices; the pose engine finishes (`0x583E0`, `0x582A0`, `0x58340`,
/// `0x59BF0`); for the table words 0, 1, `0x15` and `0x7C` six registered matrices are copied; the pose is closed
/// (`0x153240`, `0x59C00`, `[[slot+0x8C]+8] = [[slot+0x10]+0x34]`); with `flag` the model offset `[slot+4]+0x38` is
/// shifted into the pose frame by `0x96860` (relative to `[slot+0x10]+0x30` unless model flag `0x2000`). MEASURED.
pub fn shape_dynamic(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    entry: u32,
    slot: u32,
    word: u32,
    flag: u32,
) {
    let context = effects.call(memory, callee::POSE_CONTEXT, &[word]);
    let scratch = r32(memory, SCRATCH_BLOCK);
    let table = r32(memory, slot.wrapping_add(0x88));
    let saved = r32(memory, table.wrapping_add(4));
    w32(memory, table.wrapping_add(4), 0);
    let vector = entry.wrapping_sub(0x5C);
    let quaternion = entry.wrapping_sub(0x6C);
    if (r32(memory, context.wrapping_add(0x14)) as i32) > 0 {
        let mut index = 0u32;
        loop {
            let scale = model_scale(memory, r32(memory, slot.wrapping_add(4)));
            effects.call(
                memory,
                callee::POSE_BONE,
                &[context, index, 0, vector, scale],
            );
            for k in 0..4u32 {
                let word = r32(memory, vector.wrapping_add(0xC + 4 * k));
                w32(memory, quaternion.wrapping_add(4 * k), word);
            }
            let matrix = scratch.wrapping_add(index.wrapping_mul(0x40));
            effects.call(memory, callee::QUAT_MATRIX, &[matrix, quaternion]);
            for k in 0..3u32 {
                let word = r32(memory, vector.wrapping_add(4 * k));
                w32(memory, matrix.wrapping_add(0x30 + 4 * k), word);
            }
            let target = r32(
                memory,
                r32(memory, slot.wrapping_add(0x8C)).wrapping_add(0x78),
            )
            .wrapping_add(index.wrapping_mul(0x40));
            effects.call(memory, callee::MATRIX_COPY, &[target, matrix]);
            index = index.wrapping_add(1);
            if (index as i32) >= (r32(memory, context.wrapping_add(0x14)) as i32) {
                break;
            }
        }
    }
    let table = r32(memory, slot.wrapping_add(0x88));
    w32(memory, table.wrapping_add(4), saved);
    let counter = entry.wrapping_sub(0x8C);
    w32(memory, counter, 0);
    shape_done_via(memory, effects, slot, counter, scratch);
    effects.call(memory, callee::POSE_A, &[slot, 0, 1]);
    let matrices = r32(memory, slot.wrapping_add(0x10));
    effects.call(memory, callee::POSE_B, &[slot, matrices, 0, 0xFFFF_FFFF]);
    let matrices = r32(memory, slot.wrapping_add(0x10));
    effects.call(memory, callee::POSE_C, &[slot, matrices, 0, 0xFFFF_FFFF]);
    let matrices = r32(memory, slot.wrapping_add(0x10));
    effects.call(memory, callee::POSE_D, &[matrices]);
    let kind = r32(memory, r32(memory, slot.wrapping_add(0x88)).wrapping_add(4));
    if matches!(kind, 0 | 1 | 0x15 | 0x7C) {
        for (source, target) in [
            (0xCu32, 0x60u32),
            (0x10, 0x64),
            (0x2C, 0x68),
            (0x1C, 0x6C),
            (0x44, 0x70),
            (0x38, 0x74),
        ] {
            let pose = r32(memory, slot.wrapping_add(0x8C));
            let block = r32(memory, pose);
            let index = r32(memory, block.wrapping_add(source));
            let base = r32(memory, slot.wrapping_add(0x10));
            let destination = r32(memory, pose.wrapping_add(target));
            effects.call(
                memory,
                callee::MATRIX_COPY,
                &[destination, index.wrapping_shl(6).wrapping_add(base)],
            );
        }
    }
    let identity = entry.wrapping_sub(0x40);
    effects.call(memory, callee::IDENTITY, &[identity]);
    effects.call(memory, callee::POSE_E, &[identity, 1]);
    let word = r32(
        memory,
        r32(memory, slot.wrapping_add(0x10)).wrapping_add(0x34),
    );
    let pose = r32(memory, slot.wrapping_add(0x8C));
    w32(memory, pose.wrapping_add(8), word);
    if flag != 0 {
        let model = r32(memory, slot.wrapping_add(4));
        if r32(memory, model.wrapping_add(0xC)) & 0x2000 != 0 {
            effects.call(memory, callee::POSE_F, &[slot, 0, model.wrapping_add(0x38)]);
        } else {
            let base = r32(memory, slot.wrapping_add(0x10));
            let shifted = entry.wrapping_sub(0x7C);
            for k in 0..3u32 {
                let value = f32::from_bits(r32(memory, model.wrapping_add(0x38 + 4 * k)))
                    - f32::from_bits(r32(memory, base.wrapping_add(0x30 + 4 * k)));
                w32(memory, shifted.wrapping_add(4 * k), value.to_bits());
            }
            effects.call(memory, callee::POSE_F, &[slot, 0, shifted]);
            for k in 0..3u32 {
                let word = r32(memory, shifted.wrapping_add(4 * k));
                w32(memory, model.wrapping_add(0x38 + 4 * k), word);
            }
        }
    }
    effects.call(memory, callee::POSE_DONE, &[]);
}

/// `0x8FE30` as called from `0x976E0`: through the effects (the dispatcher maps it back to [`shape_done`]).
fn shape_done_via(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    slot: u32,
    counter: u32,
    matrices: u32,
) {
    effects.call(
        memory,
        super::svc_shape::callee::SHAPE_DONE,
        &[matrices, counter, slot],
    );
}

/// `0x8FAA0(out, index, parts, value)` (stack, recursive): walks the part tree from `index` (siblings `+0x4C`,
/// children `+0x4B`, stride `0xC8`): `out[index]` = the number of kind 0 parts (byte `+0x48`) before it, or `value`
/// for a part that is not of kind 0; the children (unless kind 3) get the part's own value as theirs. MEASURED.
pub fn tree_count(memory: &mut dyn GuestMemory, out: u32, index: u32, parts: u32, value: u32) {
    let mut index = index;
    loop {
        let part = parts.wrapping_add(index.wrapping_mul(0xC8));
        let mut number = value;
        if memory.read_u8(part.wrapping_add(0x48)) == 0 {
            number = 0;
            let mut at = parts;
            while at != part {
                if memory.read_u8(at.wrapping_add(0x48)) == 0 {
                    number = number.wrapping_add(1);
                }
                at = at.wrapping_add(0xC8);
            }
        }
        w32(memory, out.wrapping_add(index.wrapping_mul(4)), number);
        if (memory.read_u8(part.wrapping_add(0x4B)) as i8) >= 0
            && memory.read_u8(part.wrapping_add(0x48)) != 3
        {
            let child = sx8(memory, part.wrapping_add(0x4B));
            tree_count(memory, out, child, parts, number);
        }
        if (memory.read_u8(part.wrapping_add(0x4C)) as i8) < 0 {
            return;
        }
        index = sx8(memory, part.wrapping_add(0x4C));
    }
}

/// Table of the 20 descriptor cells `0x8FB20` fills, by running part ordinal.
const BLOCK_CELLS: [u32; 20] = [
    0xC, 0x10, 0x14, 0x18, 0x54, 0x20, 0x28, 0x38, 0x30, 0x1C, 0x24, 0x34, 0x2C, 0x40, 0x48, 0x50,
    0x3C, 0x44, 0x4C, 0x58,
];

/// `0x8FB20` (eax = `table`, esi = `buffer`; stack `block`; frame, `entry` unused): registers the parts of a model
/// block in the shape buffer. A table kind (`table[+4]`) outside 0, 1, `0x15`, `0x27`, `0x7C` clears the cells
/// `buffer + 8 ..= 0x58`. Otherwise `0x8FAA0` fills the numbering array `[buffer+0x5C]`, then the parts are walked
/// in order: until the target ordinal (kind 0: 0x13, 1: 0x15, `0x15`: 0x17, `0x7C`: 0x14, `0x27`: 0x19 minus one)
/// is reached each part's parent byte (`+0x49`) is stored into the cell of the running ordinal (`BLOCK_CELLS`), a
/// part of kind 0 advancing the ordinal; parts of kind 1 or 3 whose child parent bytes equal `[buffer+0x54]` get
/// the value 4 in the numbering array. MEASURED.
pub fn register_block(memory: &mut dyn GuestMemory, table: u32, buffer: u32, block: u32) {
    let kind = r32(memory, table.wrapping_add(4));
    if !matches!(kind, 0 | 1 | 0x15 | 0x27 | 0x7C) {
        for offset in (8..=0x58u32).step_by(4) {
            w32(memory, buffer.wrapping_add(offset), 0);
        }
        return;
    }
    let count = r32(memory, block);
    let parts = block.wrapping_sub(count.wrapping_mul(0xC8));
    let out = r32(memory, buffer.wrapping_add(0x5C));
    tree_count(memory, out, 0, parts, 0);
    let target = match kind {
        0x15 => 0x17u32,
        0x7C => 0x14,
        1 => 0x15,
        0 => 0x13,
        _ => 0x19,
    };
    let mut found = false;
    let mut ordinal = 0i32;
    if (r32(memory, block) as i32) <= 0 {
        return;
    }
    let mut part = parts.wrapping_add(0x49);
    let mut index = 0u32;
    loop {
        if !found {
            if (ordinal as u32) <= 0x13 {
                let cell = BLOCK_CELLS[ordinal as usize];
                let value = sx8(memory, part);
                w32(memory, buffer.wrapping_add(cell), value);
            }
            if memory.read_u8(part.wrapping_sub(1)) == 0 {
                let limit = (target as i32).wrapping_sub(1);
                if ordinal == limit {
                    found = true;
                } else if ordinal < limit {
                    ordinal += 1;
                }
            }
        }
        let class = memory.read_u8(part.wrapping_sub(1));
        if class == 3 || class == 1 {
            let reference = r32(memory, buffer.wrapping_add(0x54));
            let first = sx8(memory, part.wrapping_add(1)).wrapping_mul(0xC8);
            let second = sx8(memory, part.wrapping_add(2)).wrapping_mul(0xC8);
            let first = sx8(memory, first.wrapping_add(parts).wrapping_add(0x49));
            let matched = first == reference || {
                let second = sx8(memory, second.wrapping_add(parts).wrapping_add(0x49));
                second == reference
            };
            if matched {
                let out = r32(memory, buffer.wrapping_add(0x5C));
                w32(memory, out.wrapping_add(index.wrapping_mul(4)), 4);
            }
        }
        index = index.wrapping_add(1);
        part = part.wrapping_add(0xC8);
        if (index as i32) >= (r32(memory, block) as i32) {
            break;
        }
    }
}

/// `0x18FDC0(file, kind, class, include, exclude)` (stack): walks the three command streams of every part like
/// `0x18FB10`; a command whose kind byte (`+3`) sign extended equals the low word of `kind`, and whose texture row
/// either is unused (negative index) or has flags matching `include` and not `exclude`, gets the low word of `class` in
/// `+8`. MEASURED.
pub fn retag_streams(
    memory: &mut dyn GuestMemory,
    file: u32,
    kind: u32,
    class: u32,
    include: u32,
    exclude: u32,
) {
    let count = r32(memory, file) as i32;
    if count <= 0 {
        return;
    }
    let rows = r32(memory, file.wrapping_add(0x14));
    let mut base = file
        .wrapping_sub((count as u32).wrapping_mul(0xC8))
        .wrapping_add(0x60);
    let mut part = 0i32;
    loop {
        for slot in 0..3u32 {
            let mut command = r32(memory, base.wrapping_add(4 * slot));
            if command == 0 {
                continue;
            }
            while memory.read_u8(command.wrapping_add(3)) as i8 >= 0 {
                let kind_byte = i32::from(memory.read_u8(command.wrapping_add(3)) as i8);
                let index = i32::from(
                    (u16::from(memory.read_u8(command))
                        | u16::from(memory.read_u8(command.wrapping_add(1))) << 8)
                        as i16,
                );
                let mut matched = true;
                if index >= 0 {
                    let row = rows.wrapping_add((index as u32).wrapping_mul(0x30));
                    let flags = r32(memory, row.wrapping_add(0x1C));
                    matched = exclude & flags == 0 && include & flags != 0;
                }
                if matched && kind_byte as i16 == kind as u16 as i16 {
                    memory.write_u8(command.wrapping_add(8), class as u8);
                    memory.write_u8(command.wrapping_add(9), (class >> 8) as u8);
                }
                command = command.wrapping_add(10);
                if command == 0 {
                    break;
                }
            }
        }
        part += 1;
        base = base.wrapping_add(0xC8);
        if part >= r32(memory, file) as i32 {
            break;
        }
    }
}

/// Runs the ported shape engine services for their callee addresses and forwards the rest to `inner`. `entry` is
/// the esp at entry of the running `0x97C00` (as in `svc_shape::PortedShape`): `0x976E0`, reached through `0x97A00`
/// from it, sees `entry - 0x30` (`0x97A00` is called with three words pushed, `0x976E0` with one, MEASURED from the
/// call sites). `0xA5CE0` and `0x1B99B0` are the T1856 ports (`svc_spawn_c1950`, goldens `spawn_a5ce0`), `0x6A540`
/// is `svc_reloc::light_slot`.
pub struct PortedShapeServices<'a> {
    pub inner: &'a mut dyn DestroyEffects,
    pub entry: u32,
}

impl DestroyEffects for PortedShapeServices<'_> {
    fn call(&mut self, memory: &mut dyn GuestMemory, address: u32, args: &[u32]) -> u32 {
        use super::svc_shape::callee as shape;
        let arg = |index: usize| args.get(index).copied().unwrap_or(0);
        match address {
            shape::SHAPE_DONE => shape_done(memory, self, arg(2), arg(1), arg(0)),
            shape::SHAPE_DYNAMIC => {
                let entry = self.entry.wrapping_sub(0x30);
                shape_dynamic(memory, self, entry, arg(2), arg(1), arg(0));
            }
            shape::SHAPE_REGISTER_BLOCK => {
                register_block(memory, arg(1), arg(2), arg(0));
            }
            0x18_FDC0 => retag_streams(memory, arg(0), arg(1), arg(2), arg(3), arg(4)),
            shape::LIGHT_SLOT => return super::svc_reloc::light_slot(memory),
            shape::WEAPON_RECORD => return super::svc_reloc::weapon_class(memory, arg(0)),
            shape::DETACH => super::svc_spawn_c1950::set_callback(memory, arg(0), arg(1), arg(2)),
            _ if super::svc_pose::is_pose_callee(address) => {
                let mut pose = super::svc_pose::PosePorts {
                    inner: &mut *self.inner,
                };
                return pose.call(memory, address, args);
            }
            _ => return self.inner.call(memory, address, args),
        }
        0
    }
}
