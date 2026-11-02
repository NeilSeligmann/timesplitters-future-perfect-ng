//! Native dependency-list prefix for animation resource loading.
//!
//! The source owns the borrowed list and name table. Reads intentionally go
//! through the source for each operation: the original loop rereads its count
//! and row index, including after a mount callback. This models that behavior
//! without retaining guest pointers or assuming callback immutability.

use alloc::vec::Vec;

/// Original resource-list row that suppresses the fallback after its unique
/// name has actually been attempted.
pub const FALLBACK_SUPPRESSING_INDEX: u32 = 0x38;
/// The fixed fallback name is the name-table entry at `0x4FA17C`.
pub const FALLBACK_NAME_INDEX: u32 = 1;
/// Levels above this signed value return before requesting the dependency list.
pub const MAX_PREFIX_LEVEL: i32 = 0x64;

/// Read-only access to the list and runtime name table, plus the ordered mount
/// callback. Methods take `&mut self` because mount callbacks may alter the
/// list or name table; later reads observe those changes as the original does.
pub trait DependencyWorld {
    type MountResult;

    /// Whether the initial list service returned a non-null list. A null list
    /// bypasses both the prefix and fallback.
    fn list_present(&mut self) -> bool;
    /// Signed list count, reread on every loop-condition check.
    fn list_count(&mut self) -> i32;
    /// Dword resource index for a valid row in the current list.
    fn list_index(&mut self, row: i32) -> u32;
    /// Runtime name bytes for a resource-table index; `None` is a null name.
    /// Bytes omit the C terminator and are compared exactly.
    fn name_bytes(&mut self, index: u32) -> Option<Vec<u8>>;
    /// Runtime bytes at the fixed fallback row (`0x4FA17C`), excluding NUL.
    fn fallback_name(&mut self) -> Vec<u8>;
    /// Attempt a mount. Its result is recorded but never controls fallback.
    fn mount(&mut self, resource_index: u32, name: &[u8]) -> Self::MountResult;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrefixDisposition {
    /// The signed level gate returned before asking for a dependency list.
    LevelExcluded,
    /// The list service returned null; the original also skips fallback here.
    NullList,
    /// A non-null list was processed. The fallback flag records whether its
    /// unique row was attempted and whether fallback was attempted afterward.
    ListProcessed {
        suppressing_row_attempted: bool,
        fallback_attempted: bool,
    },
}

/// One observed mount request. `row == None` identifies the fixed fallback.
/// The callback result is independent of whether the suppressing row was seen.
#[derive(Debug, PartialEq, Eq)]
pub struct MountAttempt<R> {
    pub row: Option<i32>,
    pub name: Vec<u8>,
    pub result: R,
}

#[derive(Debug, PartialEq, Eq)]
pub struct DependencyRun<R> {
    pub disposition: PrefixDisposition,
    pub attempts: Vec<MountAttempt<R>>,
}

/// Execute the dependency-list prefix for a signed current level.
///
/// The list count and indices are deliberately reread around mount callbacks.
/// Current index zero and null names are skipped. Duplicate suppression compares
/// name bytes against earlier listed rows (including an earlier index-zero row),
/// so a duplicate row carrying [`FALLBACK_SUPPRESSING_INDEX`] does not suppress
/// fallback unless that row itself is the unique row whose name was attempted.
pub fn run_dependency_prefix<W: DependencyWorld>(
    level: i32,
    world: &mut W,
) -> DependencyRun<W::MountResult> {
    let mut attempts = Vec::new();
    if level > MAX_PREFIX_LEVEL {
        return DependencyRun {
            disposition: PrefixDisposition::LevelExcluded,
            attempts,
        };
    }
    if !world.list_present() {
        return DependencyRun {
            disposition: PrefixDisposition::NullList,
            attempts,
        };
    }

    let mut row = 0i32;
    let mut suppressing_row_attempted = false;
    while row < world.list_count() {
        let index = world.list_index(row);
        if index != 0 {
            if let Some(name) = world.name_bytes(index) {
                let mut duplicate = false;
                for earlier_row in 0..row {
                    let earlier_index = world.list_index(earlier_row);
                    if let Some(earlier_name) = world.name_bytes(earlier_index) {
                        if earlier_name == name {
                            duplicate = true;
                            break;
                        }
                    }
                }

                if !duplicate {
                    let result = world.mount(index, &name);
                    attempts.push(MountAttempt {
                        row: Some(row),
                        name,
                        result,
                    });

                    // The source rereads the current row after the callback;
                    // a mount result does not affect this marker.
                    if world.list_index(row) == FALLBACK_SUPPRESSING_INDEX {
                        suppressing_row_attempted = true;
                    }
                }
            }
        }
        row += 1;
    }

    let fallback_attempted = !suppressing_row_attempted;
    if fallback_attempted {
        let name = world.fallback_name();
        let result = world.mount(FALLBACK_NAME_INDEX, &name);
        attempts.push(MountAttempt {
            row: None,
            name,
            result,
        });
    }

    DependencyRun {
        disposition: PrefixDisposition::ListProcessed {
            suppressing_row_attempted,
            fallback_attempted,
        },
        attempts,
    }
}
