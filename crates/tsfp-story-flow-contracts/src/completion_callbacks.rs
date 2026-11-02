//! Typed model of the objective completion callback pair at lifted `0x2274C0`.
//!
//! The original loads the first callback before invoking it, then rereads the second callback
//! after the first returns. This module models that ordering for Tier 0/1 work; it is not wired
//! into the adopted Story event engine and does not claim natural runtime validation.

/// Opaque identity of a callback registered on an objective slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompletionCallback(u32);

impl CompletionCallback {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u32 {
        self.0
    }
}

/// The ordered `+0x38` then `+0x3c` callback fields of one objective slot.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CompletionCallbacks {
    first: Option<CompletionCallback>,
    second: Option<CompletionCallback>,
}

impl CompletionCallbacks {
    pub const fn new(
        first: Option<CompletionCallback>,
        second: Option<CompletionCallback>,
    ) -> Self {
        Self { first, second }
    }

    pub const fn first(&self) -> Option<CompletionCallback> {
        self.first
    }

    pub const fn second(&self) -> Option<CompletionCallback> {
        self.second
    }

    pub fn set_first(&mut self, callback: Option<CompletionCallback>) {
        self.first = callback;
    }

    pub fn set_second(&mut self, callback: Option<CompletionCallback>) {
        self.second = callback;
    }

    /// Invoke the preloaded first callback, then reread and invoke the second callback.
    ///
    /// The callback receives mutable access to the pair so a first handler can update the second
    /// field before it is loaded, matching the measured pointer-read boundary.
    pub fn dispatch(
        &mut self,
        slot: u32,
        mut invoke: impl FnMut(u32, CompletionCallback, &mut Self),
    ) {
        let first = self.first;
        if let Some(callback) = first {
            invoke(slot, callback, self);
        }

        let second = self.second;
        if let Some(callback) = second {
            invoke(slot, callback, self);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CompletionCallback as C, CompletionCallbacks};

    #[test]
    fn first_callback_is_preloaded_and_second_is_reread_after_return() {
        let mut callbacks = CompletionCallbacks::new(Some(C::new(1)), Some(C::new(2)));
        let mut calls = [(0, 0); 2];
        let mut call_count = 0;

        callbacks.dispatch(7, |slot, callback, pair| {
            calls[call_count] = (slot, callback.value());
            call_count += 1;
            if callback.value() == 1 {
                pair.set_first(Some(C::new(9)));
                pair.set_second(Some(C::new(3)));
            }
        });

        assert_eq!(call_count, 2);
        assert_eq!(calls, [(7, 1), (7, 3)]);
        assert_eq!(callbacks.first(), Some(C::new(9)));
        assert_eq!(callbacks.second(), Some(C::new(3)));
    }

    #[test]
    fn null_callbacks_are_skipped_independently() {
        let mut callbacks = CompletionCallbacks::new(None, Some(C::new(4)));
        let mut calls = [(0, 0); 1];
        let mut call_count = 0;
        callbacks.dispatch(2, |slot, callback, _| {
            calls[call_count] = (slot, callback.value());
            call_count += 1;
        });
        assert_eq!(call_count, 1);
        assert_eq!(calls, [(2, 4)]);
    }
}
