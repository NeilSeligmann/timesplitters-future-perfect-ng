//! Original weapon-owner context gate at 0x1B7CC0.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WeaponContextFacts {
    pub weapon_record: u32,
    pub player_index: i32,
    pub player_extension: u32,
    pub definition_index: u32,
}

/// The original requires a non-sentinel weapon record, an active player slot,
/// an initialized player extension, and a non-sentinel ammo definition.
pub fn has_weapon_context(facts: WeaponContextFacts) -> bool {
    facts.weapon_record != u32::MAX
        && facts.player_index >= 0
        && facts.player_extension != 0
        && facts.definition_index != u32::MAX
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference(f: WeaponContextFacts) -> bool {
        if f.weapon_record == u32::MAX {
            return false;
        }
        if f.player_index < 0 {
            return false;
        }
        if f.player_extension == 0 {
            return false;
        }
        f.definition_index != u32::MAX
    }

    #[test]
    fn tier1_weak_differential_against_rea_contract_transliteration() {
        let mut seed = 0x1b7c_c024u32;
        for _ in 0..4000 {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let weapon_record = match seed & 7 {
                0 => u32::MAX,
                1 => 0,
                _ => seed,
            };
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let player_index = match seed & 7 {
                0 => -1,
                1 => 0,
                2 => 1,
                _ => (seed >> 8) as i32,
            };
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let player_extension = if seed & 3 == 0 { 0 } else { seed };
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let definition_index = if seed & 7 == 0 { u32::MAX } else { seed };
            let facts = WeaponContextFacts {
                weapon_record,
                player_index,
                player_extension,
                definition_index,
            };
            assert_eq!(has_weapon_context(facts), reference(facts));
        }
    }

    #[test]
    fn mutation_checks_cover_each_context_requirement() {
        let valid = WeaponContextFacts {
            weapon_record: 3,
            player_index: 0,
            player_extension: 1,
            definition_index: 4,
        };
        assert!(has_weapon_context(valid));
        assert!(!has_weapon_context(WeaponContextFacts {
            weapon_record: u32::MAX,
            ..valid
        }));
        assert!(!has_weapon_context(WeaponContextFacts {
            player_index: -1,
            ..valid
        }));
        assert!(!has_weapon_context(WeaponContextFacts {
            player_extension: 0,
            ..valid
        }));
        assert!(!has_weapon_context(WeaponContextFacts {
            definition_index: u32::MAX,
            ..valid
        }));
    }
}
