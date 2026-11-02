// SPDX-License-Identifier: GPL-3.0-or-later
//! Input commands and the native focus retained while a child is drawn.

use std::collections::VecDeque;
use std::ops::Deref;
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FocusIdentity {
    pub page: u32,
    pub child: u32,
    pub first: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Focus {
    pub identity: FocusIdentity,
    pub index: u32,
    pub column: i32,
    pub node: u32,
}

#[derive(Debug)]
pub struct CommandQueue<T> {
    commands: Arc<Mutex<VecDeque<T>>>,
    pub focus: Arc<Mutex<Option<Focus>>>,
}

impl<T> Default for CommandQueue<T> {
    fn default() -> Self {
        Self {
            commands: Arc::default(),
            focus: Arc::default(),
        }
    }
}

impl<T> Clone for CommandQueue<T> {
    fn clone(&self) -> Self {
        Self {
            commands: self.commands.clone(),
            focus: self.focus.clone(),
        }
    }
}

impl<T> Deref for CommandQueue<T> {
    type Target = Mutex<VecDeque<T>>;
    fn deref(&self) -> &Self::Target {
        &self.commands
    }
}

impl<T> From<VecDeque<T>> for CommandQueue<T> {
    fn from(commands: VecDeque<T>) -> Self {
        Self {
            commands: Arc::new(Mutex::new(commands)),
            ..Self::default()
        }
    }
}

/// Discard stale focus when the active page, child or row list changes.
pub fn synchronize(focus: &mut Option<Focus>, identity: Option<FocusIdentity>) {
    if focus.is_some_and(|f| Some(f.identity) != identity) {
        *focus = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_is_discarded_on_close_and_topology_change() {
        let id = FocusIdentity {
            page: 1,
            child: 2,
            first: 3,
        };
        let value = Some(Focus {
            identity: id,
            index: 1,
            column: 0,
            node: 4,
        });
        let mut focus = value;
        synchronize(&mut focus, Some(id));
        assert_eq!(focus, value);
        for changed in [
            None,
            Some(FocusIdentity { page: 5, ..id }),
            Some(FocusIdentity { child: 5, ..id }),
            Some(FocusIdentity { first: 5, ..id }),
        ] {
            let mut focus = value;
            synchronize(&mut focus, changed);
            assert_eq!(focus, None);
        }
    }
}
