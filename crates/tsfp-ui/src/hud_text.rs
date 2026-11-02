// SPDX-License-Identifier: GPL-3.0-or-later
//! T1909 HUD text path, new module (does not touch the T1891 `hud` tree). Ports are readable
//! translations of the original bytes, verified against them by `tools/hud_oracle.py` (Unicorn on
//! seeded random guest state) through `tests/hud_text_oracle.rs`.
use crate::memory::GuestMemory;

// Canonical implementations: 0x40C10 lives here. 0x3FA10 and 0x42490 are canonical in `hud::textpath`
// (T1891/T1909 bulk path); the versions below are a second independent port checked by
// `tests/hud_text_oracle.rs` (see docs/t-hud-slice.md section 7.3).

/// Calls the text path makes into other game code, recorded as (callee VA, stack arguments).
pub trait TextCalls {
    fn call(&mut self, callee: u32, args: &[u32]);
}

/// Test/diagnostic implementation: keeps the calls in order.
#[derive(Default, Debug, PartialEq, Eq)]
pub struct RecordedCalls(pub Vec<(u32, Vec<u32>)>);
impl TextCalls for RecordedCalls {
    fn call(&mut self, callee: u32, args: &[u32]) {
        self.0.push((callee, args.to_vec()));
    }
}

const FONT_MODE: u32 = 0x006b_93a8;
const CURRENT_FONT: u32 = 0x006b_93b8;
const CURRENT_FONT_OBJECT: u32 = 0x006b_93bc;
const CURRENT_FONT_INDEX: u32 = 0x006b_93b4;
const FONT_OBJECTS: u32 = 0x004c_8de8; // 20 bytes per font object
const FONT_TABLE: u32 = 0x004c_8c28; // 0x38 bytes per font
const FONT_TABLE_ALTERNATE: u32 = 0x004c_8c98; // font used when the mode flag is set
const SCALE_UPDATE: u32 = 0x0003_f8d0;
const FONT_OBJECT_STRIDE: u32 = 20;
const FONT_RECORD_STRIDE: u32 = 0x38;

/// Address of the selected font object's first word in the guest table.
fn font_object(index: u32) -> u32 {
    FONT_OBJECTS.wrapping_add(index.wrapping_mul(FONT_OBJECT_STRIDE))
}

/// The object's first word selects a record in the normal font table.
fn font_record(record_index: u32) -> u32 {
    FONT_TABLE.wrapping_add(record_index.wrapping_mul(FONT_RECORD_STRIDE))
}

/// 0x3FA10 `game_hud_text_scale_globals_update_from_font_table` (called `font exchange` by the
/// HUD text code; 151 callers). cdecl(index): selects font object `index`, then the font record
/// (the alternate record when the mode flag is set, else record `[object]` of the 0x38 byte table),
/// refreshes the text scale globals (0x3F8D0) and returns the previously selected index (EAX).
// portreg: va=0x0003fa10 tier=1
// portreg: mirrors=tsfp-ports/c2r/ui_framework
pub fn font_exchange<M: GuestMemory, C: TextCalls>(m: &mut M, calls: &mut C, index: u32) -> u32 {
    let alternate = m.read32(FONT_MODE) != 0;
    let previous = m.read32(CURRENT_FONT_INDEX);
    m.write32(CURRENT_FONT_INDEX, index);
    let object = font_object(index);
    m.write32(CURRENT_FONT_OBJECT, object);
    let font = if alternate {
        FONT_TABLE_ALTERNATE
    } else {
        font_record(m.read32(object))
    };
    m.write32(CURRENT_FONT, font);
    calls.call(SCALE_UPDATE, &[]);
    previous
}

/// The string layout core 0x40FA0 call made by [`measure_text`]. `x`/`y` are in/out through
/// pointers in the original (stack floats); the context struct passed by pointer has only its
/// first dword (`first`) and sixth dword (`sixth`, zero) initialised by the caller, the other
/// words are stack residue and not part of the contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StringCoreCall {
    pub flags: u32,
    pub text: u32,
    pub callback: u32,
    pub first: u32,
    pub sixth: u32,
}

/// Game code `measure_text` calls: the render state setter (0x19650) and the string core (0x40FA0).
pub trait TextCore {
    /// 0x19650(group): selects the render state group, returns the previous one.
    fn render_state(&mut self, group: u32) -> u32;
    /// 0x40FA0: lays out a string starting at (`x`, `y`), updating both in place.
    fn string_core(&mut self, call: &StringCoreCall, x: &mut f32, y: &mut f32);
}

/// x86 `cvttss2si`: truncates toward zero, NaN and out of range give the "integer indefinite".
pub fn cvttss2si(value: f32) -> u32 {
    if value.is_nan() || value >= 2_147_483_648.0 || value < -2_147_483_648.0 {
        0x8000_0000
    } else {
        value as i32 as u32
    }
}

/// 0x42490 `text measure`: cdecl(x_ptr, y_ptr, text, context). Converts the integers at `x_ptr` and
/// `y_ptr` to floats, runs the string core with the HUD render state group 0xC selected (the
/// callback 0x40260 draws the textured rect), restores the render state and stores the resulting
/// positions back as truncated integers (x first, then y).
// portreg: va=0x00042490 tier=1
pub fn measure_text<M: GuestMemory, C: TextCore>(
    m: &mut M,
    core: &mut C,
    x_ptr: u32,
    y_ptr: u32,
    text: u32,
    context: u32,
) {
    let mut x = m.read32(x_ptr) as i32 as f32;
    let mut y = m.read32(y_ptr) as i32 as f32;
    let previous = core.render_state(0xC);
    core.string_core(
        &StringCoreCall {
            flags: 0,
            text,
            callback: 0x0004_0260,
            first: context,
            sixth: 0,
        },
        &mut x,
        &mut y,
    );
    core.render_state(previous);
    m.write32(x_ptr, cvttss2si(x));
    m.write32(y_ptr, cvttss2si(y));
}

/// Game code the word wrap (0x40C10) calls.
pub trait WrapCore {
    /// 0x3FC90(flag) with EAX = pointer to the text after an inline `,` code: parses the code and
    /// returns it (0xB2 and 0xB8 end the current token without text).
    fn parse_code(&mut self, text_ptr: u32, flag: u32) -> u32;
    /// 0x3FCF0(&width, &height, buffer, flag): layout size of the NUL terminated `text`.
    fn measure(&mut self, text: &[u8], flag: u32) -> (f32, f32);
}

const INLINE_CODES: u32 = 0x006b_93a8;
const MEASURE_ENABLED: u32 = 0x007e_7278;
const KEEP_SPACES: u32 = 0x007e_7284;
/// Stack buffer of the original (0x1F3 byte limit before the guard truncates the token).
const TOKEN_LIMIT: usize = 0x1f3;

/// The token buffer as the layout core sees it: a C string, so it ends at the first NUL even when
/// the scanned length is longer (a 0x0B escape at the end of the text copies the NUL as its operand).
fn token(buffer: &[u8], length: usize) -> &[u8] {
    let end = buffer[..length]
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(length);
    &buffer[..end]
}

/// 0x40C10 `text wrap`: cdecl(text, width, out). Copies `text` into `out`, inserting `\n` where a
/// line would exceed `width`. A zero `width` copies the string unchanged. Tokens are words, single
/// spaces and newlines; an explicit newline is copied as a hard line event and resets the current
/// line width. Soft wraps select a space boundary and reset that width before measuring the next
/// line. The inline code `,<digits>,` (when [`INLINE_CODES`] is set) and 0x0B pairs belong to the
/// word. Token widths come from the layout core (0x3FCF0) when 0x7E7278 is set.
// portreg: va=0x00040c10 tier=1
pub fn wrap_text<M: GuestMemory, C: WrapCore>(
    m: &mut M,
    core: &mut C,
    text: u32,
    width: f32,
    out: u32,
) {
    m.write8(out, 0);
    if width == 0.0 {
        let mut from = text;
        let mut to = out;
        loop {
            let byte = m.read8(from);
            m.write8(to, byte);
            from = from.wrapping_add(1);
            to = to.wrapping_add(1);
            if byte == 0 {
                return;
            }
        }
    }
    let mut src = text;
    let mut dst = out;
    let mut current = m.read8(src);
    if current == 0 {
        m.write8(dst, 0);
        return;
    }
    let mut line_width = 0.0f32;
    let mut line_start = dst;
    let mut buffer = [0u8; 0x210];
    loop {
        let mut length = 0usize; // esi
        let mut state = 0u32; // edi: 0 scanning, 1 token complete, 2 overflow break
        let mut break_pos: i32 = -1;
        let mut skip_scan = false;
        if current == 0x0a {
            dst = dst.wrapping_add(1);
            m.write8(dst.wrapping_sub(1), 0x0a);
            src = src.wrapping_add(1);
            line_width = 0.0;
            m.write8(dst, 0);
            line_start = dst;
            buffer[0] = 0;
            skip_scan = true;
        } else if current == 0x20 {
            buffer[0] = 0x20;
            buffer[1] = 0;
            length = 1;
            skip_scan = true;
        }
        if !skip_scan {
            loop {
                let codes = m.read32(INLINE_CODES) != 0;
                let byte = m.read8(src.wrapping_add(length as u32));
                if codes && byte == b',' {
                    buffer[length] = byte;
                    length += 1;
                    let mut next = m.read8(src.wrapping_add(length as u32));
                    let mut cursor = src.wrapping_add(length as u32);
                    if next != 0 {
                        loop {
                            let digit = i32::from(next as i8) - 0x30;
                            if !(0..10).contains(&digit) {
                                break;
                            }
                            buffer[(cursor.wrapping_sub(src)) as usize] = next;
                            next = m.read8(cursor.wrapping_add(1));
                            length += 1;
                            cursor = cursor.wrapping_add(1);
                            if next == 0 {
                                break;
                            }
                        }
                    }
                    state = 1;
                    let mut terminate = true;
                    if next != 0 && next == b',' {
                        let code = core.parse_code(src.wrapping_add(length as u32), 0);
                        if code == 0xb2 || code == 0xb8 {
                            state = 0;
                            terminate = false;
                        }
                    }
                    if terminate {
                        buffer[length] = 0;
                    }
                } else {
                    if byte == 0 || byte == 0x20 || byte == 0x0a {
                        state = 1;
                    } else if byte == 0x0b {
                        buffer[length] = byte;
                        let second = m.read8(src.wrapping_add(length as u32 + 1));
                        length += 1;
                        buffer[length] = second;
                        length += 1;
                    } else {
                        buffer[length] = byte;
                        length += 1;
                    }
                    buffer[length] = 0;
                    if state == 0 && m.read32(MEASURE_ENABLED) != 0 {
                        let (token_width, _) = core.measure(token(&buffer, length), 0);
                        if token_width + line_width > width && break_pos == -1 {
                            break_pos = length as i32 - 1;
                        }
                        if token_width > width && break_pos >= 1 {
                            length = break_pos as usize;
                            state = 2;
                        }
                    }
                }
                if length >= TOKEN_LIMIT {
                    buffer[length - 1] = 0;
                }
                if state != 0 {
                    break;
                }
            }
        }
        // token finished
        let mut finish_state = state;
        if !skip_scan && state == 2 {
            for _ in 0..length {
                let byte = m.read8(src);
                m.write8(dst, byte);
                dst = dst.wrapping_add(1);
                src = src.wrapping_add(1);
            }
            m.write8(dst, 0x0a);
            dst = dst.wrapping_add(1);
            line_width = 0.0;
            line_start = dst;
            finish_state = 2;
        } else {
            let (token_width, _) = core.measure(token(&buffer, length), 0);
            let mut copy = true;
            if token_width + line_width > width {
                let minimum = out.wrapping_add(1);
                let mut flag = m.read32(KEEP_SPACES);
                while dst >= minimum && m.read8(dst.wrapping_sub(1)) == 0x20 && flag == 0 {
                    dst = dst.wrapping_sub(1);
                }
                if dst != line_start {
                    m.write8(dst, 0x0a);
                    flag = m.read32(KEEP_SPACES);
                    dst = dst.wrapping_add(1);
                }
                line_width = 0.0;
                line_start = dst;
                if length == 1 && buffer[0] == 0x20 && flag == 0 {
                    src = src.wrapping_add(1);
                    copy = false;
                }
            }
            if copy {
                for _ in 0..length {
                    let byte = m.read8(src);
                    m.write8(dst, byte);
                    dst = dst.wrapping_add(1);
                    src = src.wrapping_add(1);
                }
            }
        }
        m.write8(dst, 0);
        if finish_state != 2 {
            let (token_width, _) = core.measure(token(&buffer, length), 0);
            line_width += token_width;
        }
        current = m.read8(src);
        if current == 0 {
            m.write8(dst, 0);
            return;
        }
    }
}
