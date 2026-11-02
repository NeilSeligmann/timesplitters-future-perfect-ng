// SPDX-License-Identifier: GPL-3.0-or-later
//! T2238 typed scene collection; original 0x86C40 and 0x9FF10 contracts.
use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoomCandidate {
    pub room: u32,
    pub depth: i32,
    pub visible: bool,
}
/// Original room choice scans only multi-room objects, takes the first minimum
/// visible nonnegative depth below999, and falls back when no positive ID won.
/// This changes render metadata, never simulation positions or camera state.
pub fn choose_room(fallback: u32, multiple: bool, candidates: &[RoomCandidate]) -> u32 {
    if !multiple {
        return fallback;
    }
    let mut best_depth = 999;
    let mut best_room = 0;
    for candidate in candidates {
        if candidate.visible && candidate.depth >= 0 && candidate.depth < best_depth {
            best_depth = candidate.depth;
            best_room = candidate.room;
        }
    }
    if best_room as i32 > 0 {
        best_room
    } else {
        fallback
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrawRoom {
    pub room: u8,
    pub depth: i32,
    pub after_ordinary: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomOrder {
    /// Native room IDs in original per-depth order. No linked guest arrays.
    pub buckets: Vec<Vec<u8>>,
    pub draw_max_depth: i32,
    pub restore_max_depth: i32,
    pub deferred: Vec<Vec<u8>>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollectError {
    InvalidDepth,
    TooManyRooms,
}
/// Original 80 ordinary buckets (200 rooms each) and 80 deferred buckets
/// (10 rooms each). Deferred groups append after ordinary max_depth in depth
/// order; excess deferred members are ignored as the original explicitly does.
pub fn collect_rooms(rooms: &[DrawRoom], max_depth: i32) -> Result<RoomOrder, CollectError> {
    if max_depth >= 80 {
        return Err(CollectError::InvalidDepth);
    }
    let mut ordinary = alloc::vec![Vec::new();80];
    let mut deferred = alloc::vec![Vec::new();80];
    for room in rooms {
        if room.depth < 0 {
            continue;
        }
        let depth = room.depth as usize;
        if depth >= 80 {
            return Err(CollectError::InvalidDepth);
        }
        if room.after_ordinary {
            if deferred[depth].len() < 10 {
                deferred[depth].push(room.room);
            }
        } else {
            if ordinary[depth].len() >= 200 {
                return Err(CollectError::TooManyRooms);
            }
            ordinary[depth].push(room.room);
        }
    }
    let mut last = max_depth;
    if max_depth >= 0 {
        for bucket in &deferred[..=max_depth as usize] {
            if bucket.is_empty() {
                continue;
            }
            last += 1;
            if last >= 80 {
                return Err(CollectError::InvalidDepth);
            }
            ordinary[last as usize] = bucket.clone();
        }
    }
    Ok(RoomOrder {
        buckets: ordinary,
        draw_max_depth: last.min(79),
        restore_max_depth: max_depth,
        deferred,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    #[test]
    fn visible_minimum_keeps_first_tie_and_fallback() {
        let candidates = [
            RoomCandidate {
                room: 5,
                depth: 2,
                visible: true,
            },
            RoomCandidate {
                room: 7,
                depth: 2,
                visible: true,
            },
            RoomCandidate {
                room: 8,
                depth: 0,
                visible: false,
            },
        ];
        assert_eq!(choose_room(9, true, &candidates), 5);
        assert_eq!(choose_room(9, false, &candidates), 9);
        assert_eq!(
            choose_room(
                9,
                true,
                &[RoomCandidate {
                    room: 0,
                    depth: 0,
                    visible: true
                }]
            ),
            9
        );
    }
    #[test]
    fn deferred_groups_append_after_original_max_without_advancing_input() {
        let input = vec![
            DrawRoom {
                room: 1,
                depth: 0,
                after_ordinary: false,
            },
            DrawRoom {
                room: 2,
                depth: 0,
                after_ordinary: true,
            },
            DrawRoom {
                room: 3,
                depth: 2,
                after_ordinary: true,
            },
        ];
        let order = collect_rooms(&input, 2).unwrap();
        assert_eq!(order.buckets[0], [1]);
        assert_eq!(order.buckets[3], [2]);
        assert_eq!(order.buckets[4], [3]);
        assert_eq!(order.restore_max_depth, 2);
        assert_eq!(order.draw_max_depth, 4);
        assert_eq!(collect_rooms(&input, 2).unwrap(), order);
    }
}
