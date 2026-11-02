// SPDX-License-Identifier: GPL-3.0-or-later
// FABRICATED synthetic fixtures (patterned bytes); no retail data.
use tsfp_state::GuestMemory;
use tsfp_state::generated_views::*;
use tsfp_state::values::GuestPtr;

const BASE: u32 = 0x10000;
struct Image(Vec<u8>);
impl GuestMemory for Image {
    fn read8(&self, a: u32) -> u8 {
        self.0[(a - BASE) as usize]
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0[(a - BASE) as usize] = v;
    }
}
fn patterned(len: usize) -> Image {
    Image((0..len).map(|i| (i * 7 + 3) as u8).collect())
}

#[test]
fn object_fields_land_at_the_layout_offsets() {
    let mut m = patterned(0x2000);
    let v = TsfpObjectView(BASE);
    v.set_health(&mut m, 75.5);
    v.set_position(&mut m, [1.0, 2.0, 3.0]);
    v.set_model_id(&mut m, 0xbeef);
    assert_eq!(m.read32(BASE + 0x1cc), 75.5f32.to_bits());
    assert_eq!(m.read32(BASE + 0x68), 2.0f32.to_bits());
    assert_eq!(m.read16(BASE + 0x94), 0xbeef);
    assert_eq!(v.health(&m), 75.5);
}

#[test]
fn load_store_is_identity_and_touches_only_declared_fields() {
    for view in [TsfpObjectView(BASE).0, TsfpObjectView(BASE + 0x100).0] {
        let mut m = patterned(0x2000);
        let before = m.0.clone();
        let v = TsfpObjectView(view);
        let owned = v.load(&m);
        v.store(&mut m, &owned);
        assert_eq!(m.0, before);
        assert_eq!(v.diff(&m, &owned), None);
        let mut changed = owned;
        changed.position[1] = f32::from_bits(0x7fc0_1234); // NaN payload survives
        changed.health = -0.0;
        v.store(&mut m, &changed);
        assert_eq!(v.position(&m)[1].to_bits(), 0x7fc0_1234);
        assert_eq!(v.health(&m).to_bits(), (-0.0f32).to_bits());
        assert_eq!(v.diff(&m, &owned), Some("position"));
        let mut expect = before.clone();
        let o = (view - BASE) as usize;
        expect[o + 0x68..o + 0x6c].copy_from_slice(&0x7fc0_1234u32.to_le_bytes());
        expect[o + 0x1cc..o + 0x1d0].copy_from_slice(&(-0.0f32).to_bits().to_le_bytes());
        assert_eq!(m.0, expect);
    }
}

#[test]
fn character_mixed_widths_and_typed_pointers() {
    let mut m = patterned(0x1000);
    let c = TsfpCharacterView(BASE);
    c.set_object(&mut m, GuestPtr::new(0x4399_2f1b));
    c.set_pending_state(&mut m, -1);
    c.set_model_selector(&mut m, -2);
    c.set_flags_28(&mut m, 0x80);
    assert_eq!(m.read32(BASE), 0x4399_2f1b);
    assert_eq!(c.object(&m).0, 0x4399_2f1b);
    assert_eq!(m.read32(BASE + 0x9d0), u32::MAX);
    assert_eq!(c.model_selector(&m), -2);
    assert_eq!(m.read8(BASE + 0x28), 0x80);
    // a typed pointer resolves to the record view it points at
    let obj = TsfpObjectView(c.object(&m).0);
    assert_eq!(obj.0, 0x4399_2f1b);
    assert!(GuestPtr::<()>::new(0).is_null());
}

#[test]
fn nested_records_use_the_nested_size_as_stride() {
    let header = TsfpSetuHeaderView(BASE);
    assert_eq!(header.npc_spawn_defs(0).0, BASE + 0xc);
    assert_eq!(
        header.npc_spawn_defs(2).0,
        BASE + 0xc + 2 * TsfpSetuTripleView::SIZE
    );
    assert_eq!(TsfpSetuTripleView::SIZE, 0xc);
}

#[test]
fn diff_names_the_first_changed_field_in_offset_order() {
    let mut m = patterned(0x1000);
    let v = TsfpCharacterView(BASE);
    let owned = v.load(&m);
    let mut edited = owned;
    edited.armor = f32::from_bits(owned.armor.to_bits() ^ 1);
    edited.pending_state ^= 1;
    assert_eq!(v.diff(&m, &edited), Some("armor"));
    v.store(&mut m, &edited);
    assert_eq!(v.diff(&m, &edited), None);
}
