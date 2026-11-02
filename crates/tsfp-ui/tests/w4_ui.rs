use std::collections::HashMap;
use tsfp_ui::w4_ui::{
    DrawCall, W4Memory, hud_draw_textured_rect, return_constant_1, widget_draw_loadbar,
};

struct Fake(HashMap<u32, u32>);
impl W4Memory for Fake {
    fn read32(&self, a: u32) -> u32 {
        *self.0.get(&a).unwrap_or(&0)
    }
}
#[path = "support/rng.rs"]
mod rng;
use rng::Rng;

#[test]
fn return_constant_1_smoke() {
    for _ in 0..4000 {
        assert_eq!(return_constant_1(), 1);
    }
}

#[test]
fn rect_draws_when_inside_and_positive() {
    let mut m = Fake(HashMap::new());
    m.0.insert(0x4e8cf0, 640);
    m.0.insert(0x4e8cf4, 480);
    m.0.insert(0x100 + 0x18, 10f32.to_bits());
    m.0.insert(0x100 + 0x1c, 20f32.to_bits());
    m.0.insert(0x100 + 0x20, 5f32.to_bits());
    m.0.insert(0x100 + 0x24, 6f32.to_bits());
    m.0.insert(0x100, 0xabcd);
    let mut t = Vec::new();
    hud_draw_textured_rect(&m, 0x100, &mut t);
    assert_eq!(t.len(), 1);
    assert_eq!(t[0].callee, 0x1b7b0);
    assert_eq!(t[0].args[0], 10f32.to_bits());
    assert_eq!(t[0].args[8], 0xabcd);
    // Non-positive size: no draw.
    m.0.insert(0x100 + 0x20, 0f32.to_bits());
    let mut t2 = Vec::new();
    hud_draw_textured_rect(&m, 0x100, &mut t2);
    assert!(t2.is_empty());
}

#[test]
fn loadbar_smoke_random() {
    let mut r = Rng(0x1895_0004);
    for _ in 0..3000 {
        let mut m = Fake(HashMap::new());
        for k in [0x475d10u32, 0x49f648, 0x475cd4, 0x475c78, 0x475d08] {
            m.0.insert(k, ((r.next() % 1000) as f32).to_bits());
        }
        for off in [0xb4u32, 0xe4, 0xe8, 0xec, 0xf0, 0xf4] {
            m.0.insert(0x200 + off, ((r.next() % 2000) as f32).to_bits());
        }
        let mut t: Vec<DrawCall> = Vec::new();
        widget_draw_loadbar(&m, 0x200, &mut t);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].callee, 0x2d7f90);
        assert_eq!(t[0].args.len(), 6);
        assert_eq!(t[0].args[5], m.read32(0x200 + 0xf4));
    }
}
