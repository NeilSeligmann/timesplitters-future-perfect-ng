// SPDX-License-Identifier: GPL-3.0-or-later
//! Native HUD metrics for retained hybrid consumers. The provider owns typed
//! state; consumers ask for a player's metric, never a guest-memory override.
use core::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Copy, Debug)]
#[repr(u32)]
pub enum WeaponMetric {
    Phase,
    Flash,
    Alpha,
    Slide,
    Target,
    Cycle,
}

pub type Provider = extern "C" fn(slot: u32, metric: u32, output: *mut u32) -> i32;
static PROVIDER: AtomicUsize = AtomicUsize::new(0);
pub type RowProvider =
    extern "C" fn(slot: u32, kind: u32, operation: u32, value: u32, output: *mut u32) -> i32;
static ROW_PROVIDER: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Copy, Debug)]
#[repr(u32)]
pub enum RowKind {
    AmmoLeft,
    AmmoRight,
    Status,
}

#[derive(Clone, Copy, Debug)]
#[repr(u32)]
pub enum RowOperation {
    Peek,
    Set,
    Advance,
    Begin,
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_ports_hud_rows_install(provider: Option<RowProvider>) {
    ROW_PROVIDER.store(provider.map_or(0, |p| p as usize), Ordering::Release);
}

pub fn row(slot: u32, kind: RowKind, operation: RowOperation, value: u32) -> Option<u32> {
    if slot >= 4 {
        return None;
    }
    let provider = ROW_PROVIDER.load(Ordering::Acquire);
    if provider == 0 {
        return None;
    }
    // SAFETY: installer accepts only a process-lifetime RowProvider.
    let provider: RowProvider = unsafe { core::mem::transmute(provider) };
    let mut output = 0;
    (provider(slot, kind as u32, operation as u32, value, &mut output) != 0).then_some(output)
}

fn record_slot(record: u32) -> Option<u32> {
    let delta = record.checked_sub(0x7a6160)?;
    (delta % 0x177c == 0 && delta / 0x177c < 4).then_some(delta / 0x177c)
}

fn ammo_kind(side: u32) -> Option<RowKind> {
    match side {
        0 => Some(RowKind::AmmoLeft),
        1 => Some(RowKind::AmmoRight),
        _ => None,
    }
}

pub fn ammo_index_or_reference<M: crate::memory::GuestMemory>(m: &M, record: u32, side: u32) -> u8 {
    let native =
        record_slot(record).and_then(|slot| row(slot, ammo_kind(side)?, RowOperation::Peek, 0));
    native.map_or_else(
        || m.read8(record.wrapping_add(0x174c).wrapping_add(side)),
        |n| n as u8,
    )
}

pub fn set_ammo_or_reference<M: crate::memory::GuestMemory>(
    m: &mut M,
    record: u32,
    side: u32,
    value: u8,
) {
    let native = record_slot(record)
        .and_then(|slot| row(slot, ammo_kind(side)?, RowOperation::Set, u32::from(value)));
    if native.is_none() {
        m.write8(record.wrapping_add(0x174c).wrapping_add(side), value);
    }
}

fn viewer_slot<M: crate::memory::GuestMemory>(m: &M) -> u32 {
    m.read32(m.read32(0x7b0c7c).wrapping_add(4))
}

pub fn column_top_or_reference<M: crate::memory::GuestMemory>(m: &M, origin: u32) -> u32 {
    let slot = viewer_slot(m);
    if let Some(top) = row(slot, RowKind::Status, RowOperation::Peek, 0) {
        return top;
    }
    let reference = m.read32(origin);
    row(slot, RowKind::Status, RowOperation::Begin, reference).unwrap_or(reference)
}

pub fn set_column_or_reference<M: crate::memory::GuestMemory>(m: &mut M, origin: u32, value: u32) {
    if row(viewer_slot(m), RowKind::Status, RowOperation::Set, value).is_none() {
        m.write32(origin, value);
    }
}

pub fn advance_column_or_reference<M: crate::memory::GuestMemory>(
    m: &mut M,
    origin: u32,
    height: u32,
) {
    let top = column_top_or_reference(m, origin);
    if row(
        viewer_slot(m),
        RowKind::Status,
        RowOperation::Advance,
        height,
    )
    .is_none()
    {
        m.write32(origin, top.wrapping_add(height));
    }
}

/// Install a process-lifetime, guest-thread provider. An absent provider keeps
/// the standalone/reference lane unchanged.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_ports_hud_metric_install(provider: Option<Provider>) {
    PROVIDER.store(provider.map_or(0, |p| p as usize), Ordering::Release);
}

pub fn weapon(slot: u32, metric: WeaponMetric) -> Option<u32> {
    if slot >= 4 {
        return None;
    }
    let provider = PROVIDER.load(Ordering::Acquire);
    if provider == 0 {
        return None;
    }
    // SAFETY: only a process-lifetime Provider is stored by the installer.
    let provider: Provider = unsafe { core::mem::transmute(provider) };
    let mut value = 0;
    (provider(slot, metric as u32, &mut value) != 0).then_some(value)
}

/// Decode the retained caller's HUD-record handle at the hybrid ABI boundary.
/// Native handlers use `weapon(slot, metric)` directly. No memory access here.
pub fn weapon_record(record: u32, metric: WeaponMetric) -> Option<u32> {
    let delta = record.checked_sub(0x7a6160)?;
    if delta % 0x177c != 0 {
        return None;
    }
    weapon(delta / 0x177c, metric)
}

pub fn weapon_record_or_reference<M: crate::memory::GuestMemory>(
    m: &M,
    record: u32,
    metric: WeaponMetric,
) -> u32 {
    weapon_record(record, metric)
        .unwrap_or_else(|| m.read32(record.wrapping_add(0x1750 + metric as u32 * 4)))
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Unreadable;
    impl crate::memory::GuestMemory for Unreadable {
        fn read8(&self, _: u32) -> u8 {
            panic!("owned metric read guest lifecycle")
        }
        fn write8(&mut self, _: u32, _: u8) {
            panic!("metric wrote guest")
        }
    }
    extern "C" fn provider(slot: u32, metric: u32, out: *mut u32) -> i32 {
        assert!(slot < 4 && metric < 6);
        // SAFETY: weapon() supplies a live local output.
        unsafe { *out = slot * 16 + metric };
        1
    }
    #[test]
    fn retained_record_handle_uses_native_metric_without_guest_access() {
        tsfp_ports_hud_metric_install(Some(provider));
        for slot in 0..4 {
            let record = 0x7a6160 + slot * 0x177c;
            assert_eq!(
                weapon_record_or_reference(&Unreadable, record, WeaponMetric::Phase),
                slot * 16
            );
            assert_eq!(
                weapon_record_or_reference(&Unreadable, record, WeaponMetric::Alpha),
                slot * 16 + 2
            );
        }
        assert_eq!(weapon_record(0x7a6161, WeaponMetric::Phase), None);
        assert_eq!(weapon(4, WeaponMetric::Phase), None);
        tsfp_ports_hud_metric_install(None);
        assert_eq!(weapon(0, WeaponMetric::Phase), None);
    }
}
