// SPDX-License-Identifier: GPL-3.0-or-later
//! Controller assignment state for the SDL provider. SDL objects remain owned by
//! the presenter thread; this model never opens or closes an SDL object itself.

pub const PORTS: usize = 4;

#[cfg(feature = "sdl3")]
#[allow(unsafe_code)]
pub mod bridge;
pub mod profile;
pub mod provider;
#[cfg(feature = "sdl3")]
#[allow(unsafe_code)]
pub mod sdl;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selector {
    pub enabled: bool,
    /// Empty means any GUID. Occurrence counts matching devices before used IDs are skipped.
    pub guid: String,
    pub occurrence: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    pub id: u32,
    pub guid: String,
}

/// Choose candidates for vacant ports in the same order as controller_sdl.c.
/// Existing assignments are retained; the caller opens candidates and may retry
/// after an SDL open failure. A zero ID denotes a vacant port.
pub fn candidates(
    selectors: &[Selector; PORTS],
    assigned: [u32; PORTS],
    devices: &[Device],
) -> [u32; PORTS] {
    let mut result = assigned;
    for automatic in [false, true] {
        for port in 0..PORTS {
            let selector = &selectors[port];
            if !selector.enabled || result[port] != 0 || selector.guid.is_empty() != automatic {
                continue;
            }
            let mut occurrence = 0;
            for device in devices {
                if !selector.guid.is_empty() && selector.guid != device.guid {
                    continue;
                }
                let matches_occurrence = selector.occurrence.is_none_or(|n| n == occurrence);
                occurrence += 1;
                if matches_occurrence && device.id != 0 && !result.contains(&device.id) {
                    result[port] = device.id;
                    break;
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selector(guid: &str, occurrence: Option<usize>) -> Selector {
        Selector {
            enabled: true,
            guid: guid.into(),
            occurrence,
        }
    }

    fn device(id: u32, guid: &str) -> Device {
        Device {
            id,
            guid: guid.into(),
        }
    }

    #[test]
    fn explicit_selection_precedes_automatic_ports() {
        let selectors = [
            selector("", None),
            selector("a", Some(0)),
            Selector::default(),
            Selector::default(),
        ];
        let devices = [device(1, "a"), device(2, "b")];
        assert_eq!(candidates(&selectors, [0; PORTS], &devices), [2, 1, 0, 0]);
    }

    #[test]
    fn occurrence_counts_used_devices_and_existing_ports_stay_assigned() {
        let selectors = [
            selector("a", None),
            selector("a", Some(1)),
            selector("a", Some(0)),
            Selector::default(),
        ];
        let devices = [device(1, "a"), device(2, "a")];
        assert_eq!(candidates(&selectors, [1, 0, 0, 0], &devices), [1, 2, 0, 0]);
    }

    #[test]
    fn synthetic_hotplug_sequence_reconciles_vacant_ports() {
        let selectors = [
            selector("", None),
            selector("a", Some(0)),
            Selector::default(),
            Selector::default(),
        ];
        let initial = candidates(&selectors, [0; PORTS], &[device(1, "a")]);
        assert_eq!(initial, [0, 1, 0, 0]);
        let added = candidates(&selectors, initial, &[device(1, "a"), device(2, "b")]);
        assert_eq!(added, [2, 1, 0, 0]);
        let disconnected = [added[0], 0, 0, 0];
        assert_eq!(
            candidates(&selectors, disconnected, &[device(2, "b")]),
            [2, 0, 0, 0]
        );
        assert_eq!(
            candidates(&selectors, disconnected, &[device(2, "b"), device(3, "a")]),
            [2, 3, 0, 0]
        );
    }
}
