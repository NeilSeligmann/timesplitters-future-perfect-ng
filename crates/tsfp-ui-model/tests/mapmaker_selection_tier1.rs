//! T2384 Tier 1: readable cursor picking versus lifted 0x2F8610.
//! Inputs and guest memory are FABRICATED, finite, and within the documented caller contract.

use std::collections::BTreeMap;

use tsfp_ports::abi::{ExtRegisters, Fpu, GuestCall, Xmm};
use tsfp_ports::editor_reference::f2f8610;
use tsfp_ports::memory::GuestMemory;
use tsfp_ui_model::mapmaker_selection::{
    ChangeIndex, ChangeKey, ChangeRecord, ChangeTableView, CursorQuery, GridPosition,
    MapDocumentHandle, PickDimensions, SelectionState, TableGeneration, ViewportProjection,
    pick_change,
};

const EDITOR: u32 = 0x20_000;
const STACK: u32 = 0x80_000;

#[derive(Default)]
struct Mem(BTreeMap<u32, u8>);

impl Mem {
    fn write16(&mut self, address: u32, value: u16) {
        for (offset, byte) in value.to_le_bytes().into_iter().enumerate() {
            self.write8(address + offset as u32, byte);
        }
    }

    fn write32(&mut self, address: u32, value: u32) {
        for (offset, byte) in value.to_le_bytes().into_iter().enumerate() {
            self.write8(address + offset as u32, byte);
        }
    }
}

impl GuestMemory for Mem {
    fn read8(&self, address: u32) -> u8 {
        *self.0.get(&address).unwrap_or(&0)
    }

    fn write8(&mut self, address: u32, value: u8) {
        self.0.insert(address, value);
    }
}

#[derive(Default)]
struct NoCalls;

impl GuestCall for NoCalls {
    fn call(&mut self, address: u32, _registers: &mut ExtRegisters) {
        panic!("0x2F8610 must not call {address:#x}");
    }
}

fn regs() -> ExtRegisters {
    ExtRegisters {
        eax: 0,
        ecx: 0,
        edx: 0,
        esp: STACK,
        ebx: 0,
        ebp: 0,
        esi: 0,
        edi: 0,
        eflags: 0,
        fpu: Fpu {
            stack: [0.0; 8],
            top: 0,
            control: 0x27f,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}

fn dimensions() -> PickDimensions {
    PickDimensions {
        cell_extent: 1.0,
        centered_half_width: 0.5,
        odd_lower_y_extent: 2.0,
        odd_upper_y_extent: 1.0,
        even_lower_y_extent: 1.5,
        even_upper_y_base: 0.5,
    }
}

fn write_float(mem: &mut Mem, address: u32, value: f32) {
    mem.write32(address, value.to_bits());
}

fn oracle_pick(
    projection: ViewportProjection,
    current_layer: i32,
    dimensions: PickDimensions,
    records: &[ChangeRecord],
    captured_count: u16,
) -> u32 {
    let mut mem = Mem::default();
    let mut guest = NoCalls;
    mem.write32(0x7ba9e0, 0); // active viewport selector
    write_float(&mut mem, 0x7ba280, projection.pointer[0]);
    write_float(&mut mem, 0x7ba284, projection.pointer[1]);
    mem.write32(0x783ee0, projection.integer_origin[0] as u32);
    write_float(&mut mem, 0x783dd4, projection.scale[0]);
    write_float(&mut mem, 0x783dd8, projection.offset[0]);
    mem.write32(0x783dc4, projection.integer_origin[1] as u32);
    write_float(&mut mem, 0x783f20, projection.scale[1]);
    write_float(&mut mem, 0x783cc0, projection.offset[1]);
    mem.write32(0x7844a8, EDITOR);
    mem.write16(EDITOR + 0x306e, captured_count);
    mem.write32(EDITOR + 0x264, current_layer as u32);
    write_float(&mut mem, 0x475c78, dimensions.cell_extent);
    write_float(&mut mem, 0x475cdc, dimensions.centered_half_width);
    write_float(&mut mem, 0x47d58c, dimensions.odd_lower_y_extent);
    write_float(&mut mem, 0x475cf8, dimensions.odd_upper_y_extent);
    write_float(&mut mem, 0x49f76c, dimensions.even_lower_y_extent);
    write_float(&mut mem, 0x49f768, dimensions.even_upper_y_base);
    for (index, record) in records.iter().enumerate() {
        let base = EDITOR + 0x3070 + index as u32 * 12;
        mem.write16(base, record.position.x as u16);
        mem.write16(base + 2, record.position.y as u16);
        mem.write16(base + 4, record.layer as u16);
        mem.write8(base + 6, record.pick_shape);
        mem.write8(base + 7, record.transfer_type);
        mem.write16(base + 10, record.key.get() as u16);
    }
    let mut registers = regs();
    f2f8610(&mut mem, &mut guest, &mut registers);
    registers.eax
}

fn typed_pick(
    projection: ViewportProjection,
    current_layer: i32,
    dimensions: PickDimensions,
    records: &[ChangeRecord],
    captured_count: u16,
) -> Option<ChangeIndex> {
    let table = ChangeTableView::new(
        MapDocumentHandle::new(1),
        TableGeneration::new(1),
        captured_count,
        records,
    );
    pick_change(
        CursorQuery {
            projection,
            current_layer,
            dimensions,
        },
        table,
    )
}

fn record(x: i16, y: i16, layer: i16, shape: u8, key: i16) -> ChangeRecord {
    ChangeRecord {
        position: GridPosition { x, y },
        layer,
        pick_shape: shape,
        transfer_type: shape ^ 0x55,
        key: ChangeKey::new(key),
    }
}

fn next(seed: &mut u32) -> u32 {
    *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    *seed
}

#[test]
fn cursor_pick_matches_lifted_2f8610_for_2048_fabricated_states() {
    let dimensions = dimensions();
    let mut seed = 0x2384_1687;
    let mut comparisons = 0;
    let mut hits = 0;

    for case in 0..2048u32 {
        let count = (next(&mut seed) % 24 + 1) as usize;
        let current_layer = (next(&mut seed) % 5) as i32 - 2;
        let target_shape = match case % 3 {
            0 => 0x0b + (next(&mut seed) % 5) as u8,
            1 => 1 + 2 * (next(&mut seed) % 5) as u8,
            _ => 2 * (next(&mut seed) % 5) as u8,
        };
        let x = (next(&mut seed) % 65) as i16 - 32;
        let y = (next(&mut seed) % 65) as i16 - 32;
        let target = record(x, y, current_layer as i16, target_shape, -1);
        let [query_x, query_y] = match target_shape {
            0x0b..=u8::MAX => [f32::from(x) + 0.25, f32::from(y) + 0.25],
            value if value & 1 != 0 => [f32::from(x) + 0.25, f32::from(y) + 0.25],
            _ => [f32::from(x), f32::from(y)],
        };

        // Powers of two and quarter-unit offsets keep the inverse mapping exact in f32.
        let scale = [
            [0.5, 1.0, 2.0][(next(&mut seed) % 3) as usize],
            [0.5, 1.0, 2.0][(next(&mut seed) % 3) as usize],
        ];
        let offset = [
            (next(&mut seed) % 9) as f32 * 0.25 - 1.0,
            (next(&mut seed) % 9) as f32 * 0.25 - 1.0,
        ];
        let origin = [
            (next(&mut seed) % 17) as i32 - 8,
            (next(&mut seed) % 17) as i32 - 8,
        ];
        let projection = ViewportProjection {
            pointer: [
                (query_x - offset[0]) / scale[0] + origin[0] as f32,
                (query_y - offset[1]) / scale[1] + origin[1] as f32,
            ],
            integer_origin: origin,
            scale,
            offset,
        };

        let mut records = Vec::with_capacity(count);
        records.push(target);
        for _ in 1..count {
            let layer = (next(&mut seed) % 7) as i16 - 3;
            records.push(record(
                (next(&mut seed) % 65) as i16 - 32,
                (next(&mut seed) % 65) as i16 - 32,
                layer,
                (next(&mut seed) % 16) as u8,
                next(&mut seed) as i16,
            ));
        }
        // Every eighth case has a zero captured count; every fourth hides the target on another layer.
        let captured_count = if case % 8 == 0 { 0 } else { count as u16 };
        let current_layer = if case % 4 == 0 {
            current_layer + 20
        } else {
            current_layer
        };
        let lifted = oracle_pick(
            projection,
            current_layer,
            dimensions,
            &records,
            captured_count,
        );
        let rust = typed_pick(
            projection,
            current_layer,
            dimensions,
            &records,
            captured_count,
        )
        .map_or(u32::MAX, |index| u32::from(index.get()));
        assert_eq!(rust, lifted, "case {case}");
        comparisons += 1;
        hits += usize::from(lifted != u32::MAX);
    }

    // Each box shape checks lower-inclusive and upper-exclusive x/y edges.
    for shape in [0x0b, 1, 2] {
        let item = record(4, 7, 0, shape, 22);
        let cases = match shape {
            0x0b => [
                ([4.0, 7.0], 0),
                ([5.0, 7.0], u32::MAX),
                ([4.0, 7.0], 0),
                ([4.0, 8.0], u32::MAX),
            ],
            1 => [
                ([4.0, 5.0], 0),
                ([5.0, 5.0], u32::MAX),
                ([4.0, 5.0], 0),
                ([4.0, 8.0], u32::MAX),
            ],
            _ => [
                ([3.5, 5.5], 0),
                ([4.5, 5.5], u32::MAX),
                ([4.0, 5.5], 0),
                ([4.0, 7.5], u32::MAX),
            ],
        };
        for (point, expected) in cases {
            let projection = ViewportProjection {
                pointer: point,
                integer_origin: [0, 0],
                scale: [1.0, 1.0],
                offset: [0.0, 0.0],
            };
            let lifted = oracle_pick(projection, 0, dimensions, &[item], 1);
            let rust = typed_pick(projection, 0, dimensions, &[item], 1)
                .map_or(u32::MAX, |index| u32::from(index.get()));
            assert_eq!(lifted, expected);
            assert_eq!(rust, lifted);
            comparisons += 1;
            hits += usize::from(lifted != u32::MAX);
        }
    }

    // Overlapping matches return the first ascending index, independent of either record key.
    let overlap = [record(4, 7, 0, 0x0b, 900), record(4, 7, 0, 0x0b, -17)];
    let projection = ViewportProjection {
        pointer: [4.25, 7.25],
        integer_origin: [0, 0],
        scale: [1.0, 1.0],
        offset: [0.0, 0.0],
    };
    let lifted = oracle_pick(projection, 0, dimensions, &overlap, 2);
    let rust = typed_pick(projection, 0, dimensions, &overlap, 2)
        .map_or(u32::MAX, |index| u32::from(index.get()));
    assert_eq!(lifted, 0);
    assert_eq!(rust, lifted);
    comparisons += 1;

    assert_eq!(comparisons, 2061);
    assert!(hits > 1000, "differential should include many actual hits");
}

#[test]
fn selection_identity_is_document_and_generation_scoped() {
    let record = record(1, 2, 3, 0x0b, -7);
    let records = [record];
    let original = ChangeTableView::new(
        MapDocumentHandle::new(4),
        TableGeneration::new(9),
        1,
        &records,
    );
    let selection = original.selection_id(ChangeIndex::new(0)).unwrap();
    let mut state = SelectionState::default();
    state.set(Some(selection));
    assert_eq!(state.resolve(original).unwrap().record.key.get(), -7);

    let compacted = ChangeTableView::new(
        MapDocumentHandle::new(4),
        TableGeneration::new(10),
        1,
        &records,
    );
    assert!(state.resolve(compacted).is_none());
    assert_eq!(state.selected(), None);

    let other_document = ChangeTableView::new(
        MapDocumentHandle::new(5),
        TableGeneration::new(9),
        1,
        &records,
    );
    assert!(other_document.resolve(selection).is_none());
}
