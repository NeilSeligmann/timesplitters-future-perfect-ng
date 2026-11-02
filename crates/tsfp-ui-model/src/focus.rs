//! Focus and navigation graph: directional neighbours, wrap rules, enabled/disabled.

use crate::model::{Page, WidgetId};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}
impl Dir {
    pub const ALL: [Dir; 4] = [Dir::Up, Dir::Down, Dir::Left, Dir::Right];
    pub fn opposite(self) -> Dir {
        match self {
            Dir::Up => Dir::Down,
            Dir::Down => Dir::Up,
            Dir::Left => Dir::Right,
            Dir::Right => Dir::Left,
        }
    }
}

/// How the focusable widgets of a page are arranged for navigation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavLayout {
    /// Row-major grid with this many columns. `Grid(1)` is a vertical list,
    /// `Grid(0)` a single horizontal row.
    Grid(u16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wrap {
    /// Moving past an end continues at the other end of the same row or column.
    Wrap,
    /// Moving past an end has no neighbour.
    Clamp,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Neighbours {
    pub up: Option<WidgetId>,
    pub down: Option<WidgetId>,
    pub left: Option<WidgetId>,
    pub right: Option<WidgetId>,
}
impl Neighbours {
    pub fn get(&self, dir: Dir) -> Option<WidgetId> {
        match dir {
            Dir::Up => self.up,
            Dir::Down => self.down,
            Dir::Left => self.left,
            Dir::Right => self.right,
        }
    }
    pub fn set(&mut self, dir: Dir, to: Option<WidgetId>) {
        match dir {
            Dir::Up => self.up = to,
            Dir::Down => self.down = to,
            Dir::Left => self.left = to,
            Dir::Right => self.right = to,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FocusIssue {
    /// A neighbour names a widget that cannot take focus (absent, disabled or non-navigable).
    TargetNotFocusable {
        from: WidgetId,
        dir: Dir,
        to: WidgetId,
    },
    /// A focusable widget cannot be reached from the start focus.
    Unreachable(WidgetId),
    /// A focusable widget has no outgoing neighbour although other focusable widgets exist.
    DeadEnd(WidgetId),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FocusGraph {
    nodes: BTreeMap<WidgetId, Neighbours>,
    order: Vec<WidgetId>,
}

impl FocusGraph {
    /// Build the graph of a page: focusable widgets in page order laid on the page grid,
    /// then the explicit links (links from or to non-page widgets are kept so `validate` reports them).
    pub fn build(page: &Page) -> Self {
        let order: Vec<WidgetId> = page
            .widgets
            .iter()
            .filter(|w| w.is_focusable())
            .map(|w| w.id)
            .collect();
        let mut nodes: BTreeMap<WidgetId, Neighbours> = order
            .iter()
            .map(|id| (*id, Neighbours::default()))
            .collect();
        let columns = match page.nav {
            NavLayout::Grid(0) => order.len().max(1),
            NavLayout::Grid(c) => c as usize,
        };
        let count = order.len();
        for (index, id) in order.iter().enumerate() {
            let column = index % columns;
            let row = index / columns;
            let neighbours = nodes.get_mut(id).expect("node inserted above");
            // Column members: column, column + columns, ... ; row members: row * columns .. .
            let column_members: Vec<usize> = (column..count).step_by(columns).collect();
            let row_start = row * columns;
            let row_members: Vec<usize> = (row_start..count.min(row_start + columns)).collect();
            let wrap = page.wrap;
            let step = |members: &[usize], forward: bool| -> Option<WidgetId> {
                let position = members.iter().position(|m| *m == index)?;
                if members.len() < 2 {
                    return None;
                }
                let target = if forward {
                    if position + 1 < members.len() {
                        position + 1
                    } else if wrap == Wrap::Wrap {
                        0
                    } else {
                        return None;
                    }
                } else if position > 0 {
                    position - 1
                } else if wrap == Wrap::Wrap {
                    members.len() - 1
                } else {
                    return None;
                };
                Some(order[members[target]])
            };
            neighbours.up = step(&column_members, false);
            neighbours.down = step(&column_members, true);
            neighbours.left = step(&row_members, false);
            neighbours.right = step(&row_members, true);
        }
        for link in &page.links {
            if let Some(entry) = nodes.get_mut(&link.from) {
                entry.set(link.dir, Some(link.to));
            }
        }
        // Links whose source is not focusable are dropped silently: the source cannot hold focus.
        Self { nodes, order }
    }

    pub fn neighbours(&self, id: WidgetId) -> Option<&Neighbours> {
        self.nodes.get(&id)
    }
    pub fn step(&self, from: WidgetId, dir: Dir) -> Option<WidgetId> {
        self.nodes.get(&from).and_then(|n| n.get(dir))
    }
    /// Focusable widgets in page order.
    pub fn focusable(&self) -> &[WidgetId] {
        &self.order
    }

    /// Pairs where `a.dir == b` but `b.opposite(dir) != a`. Generated graphs have none;
    /// explicit links may introduce them on purpose.
    pub fn asymmetries(&self) -> Vec<(WidgetId, Dir, WidgetId)> {
        let mut out = Vec::new();
        for (from, neighbours) in &self.nodes {
            for dir in Dir::ALL {
                if let Some(to) = neighbours.get(dir) {
                    let back = self.nodes.get(&to).and_then(|n| n.get(dir.opposite()));
                    if back != Some(*from) {
                        out.push((*from, dir, to));
                    }
                }
            }
        }
        out
    }

    /// Check graph invariants. `start` is where focus begins.
    pub fn validate(&self, start: Option<WidgetId>) -> Vec<FocusIssue> {
        let mut issues = Vec::new();
        for (from, neighbours) in &self.nodes {
            for dir in Dir::ALL {
                if let Some(to) = neighbours.get(dir) {
                    if !self.nodes.contains_key(&to) {
                        issues.push(FocusIssue::TargetNotFocusable {
                            from: *from,
                            dir,
                            to,
                        });
                    }
                }
            }
        }
        if self.order.len() > 1 {
            for id in &self.order {
                let n = &self.nodes[id];
                if Dir::ALL.iter().all(|d| n.get(*d).is_none()) {
                    issues.push(FocusIssue::DeadEnd(*id));
                }
            }
        }
        if let Some(start) = start.or_else(|| self.order.first().copied()) {
            let mut seen = BTreeSet::new();
            let mut queue = VecDeque::from([start]);
            seen.insert(start);
            while let Some(current) = queue.pop_front() {
                if let Some(n) = self.nodes.get(&current) {
                    for dir in Dir::ALL {
                        if let Some(to) = n.get(dir) {
                            if self.nodes.contains_key(&to) && seen.insert(to) {
                                queue.push_back(to);
                            }
                        }
                    }
                }
            }
            for id in &self.order {
                if !seen.contains(id) {
                    issues.push(FocusIssue::Unreachable(*id));
                }
            }
        }
        issues
    }
}
