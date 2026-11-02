use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use tsfp_ports::memory::GuestMemory;

/// Sparse guest bytes shared with scripted callees through clones.
#[derive(Clone, Default)]
pub struct Mem(Rc<RefCell<BTreeMap<u32, u8>>>);
impl GuestMemory for Mem {
    fn read8(&self, a: u32) -> u8 {
        *self.0.borrow().get(&a).unwrap_or(&0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0.borrow_mut().insert(a, v);
    }
}
