// SPDX-License-Identifier: GPL-3.0-or-later
// Standalone harness: python3 tools/t2197_diff.py (not a Cargo integration test).
extern crate alloc;

#[path = "../src/lib.rs"]
mod model;
use model::{Event, TextField};
use std::{cell::RefCell, rc::Rc};
trait GuestMemory {
    fn read8(&self, a: u32) -> u8;
    fn write8(&mut self, a: u32, b: u8);
    fn read32(&self, a: u32) -> u32 {
        u32::from_le_bytes(std::array::from_fn(|i| self.read8(a + i as u32)))
    }
    fn write32(&mut self, a: u32, b: u32) {
        for (i, v) in b.to_le_bytes().iter().enumerate() {
            self.write8(a + i as u32, *v);
        }
    }
}
#[derive(Clone)]
struct Memory(Rc<RefCell<Vec<u8>>>);
impl GuestMemory for Memory {
    fn read8(&self, a: u32) -> u8 {
        if (0x475cac..0x475cb0).contains(&a) {
            0
        } else {
            self.0.borrow()[a as usize]
        }
    }
    fn write8(&mut self, a: u32, b: u8) {
        self.0.borrow_mut()[a as usize] = b;
    }
}
#[derive(Default)]
struct ExtRegisters {
    eax: u32,
    ebx: u32,
    ecx: u32,
    edx: u32,
    esi: u32,
    edi: u32,
    esp: u32,
    ebp: u32,
    xmm: [Xmm; 5],
    fpu: Fpu,
}
#[derive(Default, Clone, Copy)]
struct Xmm {
    lanes: [u32; 4],
}
impl Xmm {
    fn scalar(v: u32) -> Self {
        Self {
            lanes: [v, 0, 0, 0],
        }
    }
}
#[derive(Default)]
struct Fpu(Vec<f64>);
impl Fpu {
    fn push(&mut self, v: f64) {
        self.0.insert(0, v);
    }
    fn pop(&mut self) {
        self.0.remove(0);
    }
    fn st(&self, i: usize) -> f64 {
        self.0[i]
    }
    fn set_st(&mut self, i: usize, v: f64) {
        self.0[i] = v;
    }
}
fn fp_pc(_: &Fpu, v: f64) -> f64 {
    v as f32 as f64
}
fn fcmp(a: f64, b: f64) -> i32 {
    if a < b {
        1
    } else if a == b {
        0x40
    } else {
        0
    }
}
impl ExtRegisters {
    fn push(&mut self, m: &mut impl GuestMemory, v: u32) {
        self.esp -= 4;
        m.write32(self.esp, v);
    }
    fn call(&mut self, g: &mut impl GuestCall, va: u32) {
        g.call(va, self);
    }
}
trait GuestCall {
    fn call(&mut self, va: u32, r: &mut ExtRegisters);
}
fn set_lo8(v: u32, b: u32) -> u32 {
    (v & !255) | (b & 255)
}
struct Calls {
    memory: Memory,
    dirty: usize,
}
impl GuestCall for Calls {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        match va {
            0x40650 => {
                let p = self.memory.read32(r.esp);
                r.eax = (0..).find(|i| self.memory.read8(p + i) == 0).unwrap();
            }
            0x746d0 => self.dirty += 1,
            0x7d960 => {}
            0x7c1b0 => {
                self.memory.write32(r.ecx, 0x2500);
                r.eax = 0x2500;
            }
            0x7c300 => {
                game_text_input_ring_cursor_step_back_wrapping(&mut self.memory.clone(), self, r)
            }
            _ => panic!("unstubbed {va:x}"),
        }
    }
}
include!(env!("T2197_ORACLE"));
#[test]
fn differential_cursor_delete_blank() {
    let charset: Vec<u8> = (1..=255).collect();
    let mut count = 0;
    for seed in 0..4000usize {
        let length = seed % 65;
        let text: Vec<u8> = (0..length)
            .map(|i| {
                if seed % 5 == 0 {
                    b' '
                } else {
                    1 + ((seed * 17 + i * 31) % 255) as u8
                }
            })
            .collect();
        for cursor in [0, length / 2, length] {
            for event in [Event::Left, Event::Right, Event::Backspace] {
                for selected in [false, true] {
                    let mut field = TextField::new(&text, 255, &charset).unwrap();
                    for _ in cursor..length {
                        field.apply(Event::Left).unwrap();
                    }
                    if selected {
                        field.select_all();
                    }
                    let mut m = Memory(Rc::new(RefCell::new(vec![0; 0x4000])));
                    let object = 0x1000;
                    let buffer = 0x2000;
                    for (i, b) in text.iter().enumerate() {
                        m.write8(buffer + i as u32, *b);
                    }
                    m.write32(object + 0x220, buffer);
                    m.write32(object + 0x22c, cursor as u32);
                    m.write32(object + 0x228, if selected { 0x10 } else { 0 });
                    let mut calls = Calls {
                        memory: m.clone(),
                        dirty: 0,
                    };
                    let mut r = ExtRegisters {
                        esi: object,
                        ebx: object,
                        esp: 0x3f00,
                        ..Default::default()
                    };
                    match event {
                        Event::Left => game_text_input_ring_cursor_step_back_wrapping(
                            &mut m, &mut calls, &mut r,
                        ),
                        Event::Right => game_text_input_ring_cursor_step_forward_wrapping(
                            &mut m, &mut calls, &mut r,
                        ),
                        _ => game_ui_text_input_delete_char_at_cursor_and_shift_left_or_zero_fill(
                            &mut m, &mut calls, &mut r,
                        ),
                    }
                    field.apply(event).unwrap();
                    let actual: Vec<u8> = (0..)
                        .map(|i| m.read8(buffer + i))
                        .take_while(|b| *b != 0)
                        .collect();
                    assert_eq!(
                        field.text(),
                        actual,
                        "seed {seed} cursor {cursor} {event:?} selected {selected}"
                    );
                    assert_eq!(field.cursor(), m.read32(object + 0x22c) as usize);
                    r.ecx = buffer;
                    game_str_is_empty_or_spaces_ecx(&mut m, &mut r);
                    assert_eq!(field.can_confirm(), r.eax == 0);
                    count += 1;
                }
            }
        }
    }
    println!("{count} fabricated cases; four translated roots; equal");
}

#[test]
fn differential_insert() {
    let charset: Vec<u8> = (1..=255).collect();
    let mut count = 0;
    for seed in 0..4000usize {
        let length = seed % 65;
        let limit = (seed / 65) % 66;
        // Initial buffers can exceed the requested limit in legacy construction; restrict
        // this differential to the model's valid-state contract.
        if length > limit {
            continue;
        }
        let text: Vec<u8> = (0..length)
            .map(|i| 1 + ((seed + i * 31) % 255) as u8)
            .collect();
        let byte = 1 + (seed % 255) as u8;
        for cursor in [0, length / 2, length] {
            for selected in [false, true] {
                let mut field = TextField::new(&text, limit, &charset).unwrap();
                for _ in cursor..length {
                    field.apply(Event::Left).unwrap();
                }
                if selected {
                    field.select_all();
                }
                let mut m = Memory(Rc::new(RefCell::new(vec![0; 0x4000])));
                let object = 0x1000;
                let buffer = 0x2000;
                for (i, b) in text.iter().enumerate() {
                    m.write8(buffer + i as u32, *b);
                }
                m.write8(0x2500, byte);
                m.write32(object + 0x220, buffer);
                m.write32(object + 0x22c, cursor as u32);
                m.write32(object + 0x230, limit as u32);
                m.write32(object + 0x228, if selected { 0x10 } else { 0 });
                // Width disabled: both floats are zero; no renderer stubs are invoked.
                let mut calls = Calls {
                    memory: m.clone(),
                    dirty: 0,
                };
                let mut r = ExtRegisters {
                    esp: 0x3f00,
                    ..Default::default()
                };
                m.write32(r.esp + 4, object);
                game_ui_text_input_widget_step_character_by_pad_with_tick_sound_and_wrap(
                    &mut m, &mut calls, &mut r,
                );
                let _ = field.apply(Event::Character(char::from(byte)));
                let actual: Vec<u8> = (0..)
                    .map(|i| m.read8(buffer + i))
                    .take_while(|b| *b != 0)
                    .collect();
                assert_eq!(
                    field.text(),
                    actual,
                    "insert seed {seed} {cursor} {selected}"
                );
                assert_eq!(field.cursor(), m.read32(object + 0x22c) as usize);
                count += 1;
            }
        }
    }
    println!("{count} insertion cases equal");
}

/// 0x7C1B0 reads only the low 16 flag bits, so this covers its whole input domain.
#[test]
fn differential_charset_select_exhaustive() {
    use model::keyboard::{FieldFlags, select_table};
    let mut m = Memory(Rc::new(RefCell::new(vec![0; 0x4000])));
    let mut count = 0;
    for flags in 0..0x1_0000u32 {
        for out in [0u32, 0x2000] {
            m.write32(0x2000, 0);
            let mut r = ExtRegisters {
                edx: flags,
                ecx: out,
                esp: 0x3f00,
                ..Default::default()
            };
            game_font_charset_select(&mut m, &mut r);
            let table = select_table(FieldFlags(flags));
            assert_eq!(r.eax, table.labels, "flags {flags:#x}");
            assert_eq!(r.esp, 0x3f00);
            if out != 0 {
                assert_eq!(m.read32(0x2000), table.bytes, "flags {flags:#x}");
            }
            count += 1;
        }
    }
    println!("{count} charset-select cases equal");
}
