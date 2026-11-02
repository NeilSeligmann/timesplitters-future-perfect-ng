//! T2369 Tier 1 differential for the typed Story record offset against lifted 0x69980.
use std::collections::BTreeMap;
use tsfp_engine_fileio::story_progress::{StoryMode, StoryProgress, StoryProgressUpdate};
use tsfp_format_profile::Profile;
use tsfp_ports::abi::{ExtRegisters, GuestCall};
use tsfp_ports::c2r::{gameplay_player, network_live_2};
use tsfp_ports::memory::GuestMemory;

#[derive(Clone, Default)]
struct Memory(BTreeMap<u32, u8>);

impl GuestMemory for Memory {
    fn read8(&self, address: u32) -> u8 {
        self.0.get(&address).copied().unwrap_or(0)
    }

    fn write8(&mut self, address: u32, value: u8) {
        self.0.insert(address, value);
    }
}

#[derive(Clone, Copy)]
struct StoryCalls {
    ordinal: u32,
    story_record: u32,
    summary: u32,
    minimum_time: f32,
}

impl GuestCall for StoryCalls {
    fn call(&mut self, address: u32, registers: &mut ExtRegisters) {
        registers.eax = match address {
            0x1cd040 => 0,
            0x64d10 | 0x3568c0 => 1,
            0x64d60 => self.ordinal,
            0x69980 => self.story_record,
            0x69900 => self.summary,
            _ => 0,
        };
        if address == 0x17440 {
            registers.fpu.push(f64::from(self.minimum_time));
        }
    }
}

#[test]
fn story_record_address_matches_lifted_reader_for_3000_cases_tier1() {
    const STACK: u32 = 0x800000;
    for case in 0..3000u32 {
        let player = case % 4;
        let ordinal = case % 13;
        let shared_summary = case % 7 == 0;
        let mut memory = Memory::default();
        memory.write32(0x7de458, if shared_summary { 0x800000 } else { 0 });
        memory.write32(0x7bedc0 + player * 0x1f54, 0x900000 + case * 0x100);
        memory.write32(STACK, 0); // return address
        memory.write32(STACK + 4, player);
        memory.write32(STACK + 8, ordinal);

        let mut registers: ExtRegisters = unsafe { core::mem::zeroed() };
        registers.esp = STACK;
        network_live_2::game_slot_record_address(&mut memory, &mut registers);

        let summary_base = if shared_summary {
            0x6f11c0
        } else {
            0x7c0050 + player * 0x1f54
        };
        let record_offset = StoryProgress::record_offset(ordinal as u8).unwrap() as u32;
        assert_eq!(registers.eax, summary_base + record_offset, "case {case}");
    }
}

#[test]
fn typed_story_progress_update_matches_lifted_writer_for_3000_cases_tier1() {
    const STACK: u32 = 0x800000;
    const PAYLOAD: u32 = 0x500000;
    const SUMMARY: u32 = PAYLOAD + 0x1290;
    for case in 0..3000u32 {
        let ordinal = (case % 13) as u8;
        let selector = (case % 3) as u8;
        let old_time = (case % 31) as f32 + 1.0;
        let best_time = (case % 19) as f32 + 0.25;
        let minimum_time = (case % 27) as f32 / 2.0;
        let result_flags = 0x8000 | if case % 2 == 0 { 0x10000 } else { 0 };
        let mut profile = Profile {
            payload: vec![0x5a; 0x1f54],
            signature: [0x35; 20],
        };
        profile.payload[0x1ee0..0x1ee4].copy_from_slice(&2u32.to_le_bytes());
        let time_offset =
            0x1290 + 0x288 + usize::from(ordinal) * 0x28 + 0x0c + usize::from(selector) * 4;
        profile.payload[time_offset..time_offset + 4]
            .copy_from_slice(&old_time.to_bits().to_le_bytes());
        profile.payload[0x1290 + 0xbf4..0x1290 + 0xc00].copy_from_slice(
            &[0x10u32, 0x200, 0x400]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>(),
        );
        profile.payload[0x1290 + 0x918..0x1290 + 0x91c]
            .copy_from_slice(&case.wrapping_mul(0x10203).to_le_bytes());

        let update = StoryProgressUpdate {
            result_flags,
            campaign_ordinal: ordinal,
            selector,
            mode: StoryMode::SinglePlayer,
            best_time,
            minimum_time,
        };
        let mut expected = profile.clone();
        StoryProgress::apply_success(&mut expected, update).unwrap();

        let mut memory = Memory::default();
        for (offset, byte) in profile.payload.iter().copied().enumerate() {
            memory.write8(PAYLOAD + offset as u32, byte);
        }
        memory.write32(0x7de458, 0);
        memory.write32(0x7de45c, result_flags);
        memory.write8(0x7de455, 10);
        memory.write32(0x79094c, 2);
        memory.write32(0x790950, 1);
        memory.write32(0x7de30c, 8);
        memory.write8(0x7de457, selector);
        memory.write32(0x7de328, 1);
        memory.write32(0x4e8cec, 1);
        memory.write32(0x475c78, best_time.to_bits());
        memory.write32(0x4785b0, minimum_time.to_bits());
        memory.write32(0x7de470, u32::MAX);
        memory.write32(STACK, 0);

        let story_record = SUMMARY + 0x288 + u32::from(ordinal) * 0x28;
        let mut calls = StoryCalls {
            ordinal: u32::from(ordinal),
            story_record,
            summary: SUMMARY,
            minimum_time,
        };
        let mut registers: ExtRegisters = unsafe { core::mem::zeroed() };
        registers.esp = STACK;
        gameplay_player::game_player_stats_per_frame_update_unlocks_play_time_and_end_of_round_counters(
            &mut memory,
            &mut calls,
            &mut registers,
        );

        for start in [
            time_offset,
            0x1290 + 0xbf4,
            0x1290 + 0xbf8,
            0x1290 + 0xbfc,
            0x1290 + 0x918,
        ] {
            let actual = memory.read32(PAYLOAD + start as u32);
            let expected_word =
                u32::from_le_bytes(expected.payload[start..start + 4].try_into().unwrap());
            assert_eq!(actual, expected_word, "case {case}, field {start:#x}");
        }
    }
}
