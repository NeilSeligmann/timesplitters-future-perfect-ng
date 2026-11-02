//! Rust-owned identity and cursor picking for the Map Maker change table.
//!
//! The field meanings and scan order follow `docs/t1687-rea-selection.md`. The typed
//! representation is an inferred native boundary; callers must provide a current projection,
//! readable table snapshot, and source the six pick dimensions from the corresponding original
//! constants. It does not publish into the guest editor or perform selection transfer/removal.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MapDocumentHandle(u64);

impl MapDocumentHandle {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TableGeneration(u64);

impl TableGeneration {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChangeIndex(u16);

impl ChangeIndex {
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

/// A signed change key, kept separate from the unsigned table index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChangeKey(i16);

impl ChangeKey {
    pub const fn new(value: i16) -> Self {
        Self(value)
    }

    pub const fn get(self) -> i16 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridPosition {
    pub x: i16,
    pub y: i16,
}

/// Only fields established for the inline 12-byte record are represented here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChangeRecord {
    pub position: GridPosition,
    pub layer: i16,
    /// Original record byte +6; it selects the cursor-pick box shape.
    pub pick_shape: u8,
    /// Original record byte +7 is a different field, used by selection transfer.
    pub transfer_type: u8,
    pub key: ChangeKey,
}

/// A captured, borrowed table view. The count is the value captured once for this query.
#[derive(Clone, Copy, Debug)]
pub struct ChangeTableView<'a> {
    document: MapDocumentHandle,
    generation: TableGeneration,
    captured_count: u16,
    records: &'a [ChangeRecord],
}

impl<'a> ChangeTableView<'a> {
    /// `records` must contain at least `captured_count` valid entries, matching the caller
    /// obligations of the original helper. The table is borrowed only for this snapshot.
    pub fn new(
        document: MapDocumentHandle,
        generation: TableGeneration,
        captured_count: u16,
        records: &'a [ChangeRecord],
    ) -> Self {
        assert!(records.len() >= captured_count as usize);
        Self {
            document,
            generation,
            captured_count,
            records,
        }
    }

    pub fn captured_count(self) -> u16 {
        self.captured_count
    }

    pub fn record(self, index: ChangeIndex) -> Option<&'a ChangeRecord> {
        (index.0 < self.captured_count).then(|| &self.records[index.0 as usize])
    }

    pub fn selection_id(self, index: ChangeIndex) -> Option<SelectionId> {
        self.record(index)?;
        Some(SelectionId {
            document: self.document,
            generation: self.generation,
            index,
        })
    }

    pub fn resolve(self, selection: SelectionId) -> Option<SelectedChange<'a>> {
        if selection.document != self.document || selection.generation != self.generation {
            return None;
        }
        Some(SelectedChange {
            index: selection.index,
            record: self.record(selection.index)?,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SelectionId {
    document: MapDocumentHandle,
    generation: TableGeneration,
    index: ChangeIndex,
}

impl SelectionId {
    pub const fn index(self) -> ChangeIndex {
        self.index
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectedChange<'a> {
    pub index: ChangeIndex,
    pub record: &'a ChangeRecord,
}

/// Projection inputs captured from one active viewport. Integer origins stay integer until
/// converted to `f32`, matching the original `CVTSI2SS` order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewportProjection {
    pub pointer: [f32; 2],
    pub integer_origin: [i32; 2],
    pub scale: [f32; 2],
    pub offset: [f32; 2],
}

impl ViewportProjection {
    pub fn project(self) -> [f32; 2] {
        [
            (self.pointer[0] - self.integer_origin[0] as f32) * self.scale[0] + self.offset[0],
            (self.pointer[1] - self.integer_origin[1] as f32) * self.scale[1] + self.offset[1],
        ]
    }
}

/// Values loaded from the original pick constants. Keeping them in the snapshot avoids
/// inventing a relationship between the type byte and shape, or assuming a global default.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PickDimensions {
    /// `0x475C78`: cell extent used by the unit cell and thin boxes.
    pub cell_extent: f32,
    /// `0x475CDC`: half-width used by the even thin box.
    pub centered_half_width: f32,
    /// `0x47D58C`: lower-Y extent for odd thin boxes.
    pub odd_lower_y_extent: f32,
    /// `0x475CF8`: upper-Y extent for odd thin boxes.
    pub odd_upper_y_extent: f32,
    /// `0x49F76C`: lower-Y extent for even thin boxes.
    pub even_lower_y_extent: f32,
    /// `0x49F768`: base for the even thin box's upper-Y edge.
    pub even_upper_y_base: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CursorQuery {
    pub projection: ViewportProjection,
    /// Signed DWORD captured from editor context +0x264.
    pub current_layer: i32,
    pub dimensions: PickDimensions,
}

/// Returns the first ascending table index whose current-layer pick box contains the projected
/// cursor. The table count is captured in `ChangeTableView`; no record key or transfer type
/// participates in this query.
pub fn pick_change(query: CursorQuery, table: ChangeTableView<'_>) -> Option<ChangeIndex> {
    let [cursor_x, cursor_y] = query.projection.project();
    for (index, record) in table
        .records
        .iter()
        .take(table.captured_count as usize)
        .enumerate()
    {
        if i32::from(record.layer) != query.current_layer {
            continue;
        }

        let x = f32::from(record.position.x);
        let y = f32::from(record.position.y);
        let dimensions = query.dimensions;
        let (min_x, max_x, min_y, max_y) = if record.pick_shape >= 0x0b {
            (x, x + dimensions.cell_extent, y, y + dimensions.cell_extent)
        } else if record.pick_shape & 1 != 0 {
            (
                x,
                x + dimensions.cell_extent,
                y - dimensions.odd_lower_y_extent,
                y + dimensions.odd_upper_y_extent,
            )
        } else {
            (
                x - dimensions.centered_half_width,
                x + dimensions.centered_half_width,
                y - dimensions.even_lower_y_extent,
                y - dimensions.even_upper_y_base + dimensions.cell_extent,
            )
        };

        if cursor_x >= min_x && cursor_x < max_x && cursor_y >= min_y && cursor_y < max_y {
            return Some(ChangeIndex(index as u16));
        }
    }
    None
}

/// A selection owner that drops an index when its document or table generation no longer
/// resolves. It does not guess a replacement from a key because key uniqueness is unproven.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SelectionState {
    selected: Option<SelectionId>,
}

impl SelectionState {
    pub fn set(&mut self, selection: Option<SelectionId>) {
        self.selected = selection;
    }

    pub fn selected(&self) -> Option<SelectionId> {
        self.selected
    }

    pub fn resolve<'a>(&mut self, table: ChangeTableView<'a>) -> Option<SelectedChange<'a>> {
        let resolved = self.selected.and_then(|selection| table.resolve(selection));
        if resolved.is_none() {
            self.selected = None;
        }
        resolved
    }
}
