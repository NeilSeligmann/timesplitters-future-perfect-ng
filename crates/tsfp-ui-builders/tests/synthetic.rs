//! Host-free contracts: fabricated tables and opener state, never private capture data.
use tsfp_ports::memory::GuestMemory;
use tsfp_ui_builders::compare::compare_pages;
use tsfp_ui_builders::native::{Confirmation, SaveStatus};
use tsfp_ui_builders::tables;
use tsfp_ui_builders::toolkit::{Ram, Scenario, build_page};
use tsfp_ui_model::{HandlerRef, ModelRecorder, PageId, TextId, TextRef, UiModel};

fn translated(page: u64, builder: u32, scenario: &Scenario) -> UiModel {
    let mut rec = ModelRecorder::new();
    let run = build_page(&mut rec, PageId(page), builder, scenario).unwrap();
    assert!(
        run.unmodelled.is_empty(),
        "unmodelled: {:x?}",
        run.unmodelled
    );
    rec.finish().unwrap()
}

#[test]
fn native_confirmation_matches_runtime_opener_variants() {
    let page = PageId(0x524a70);
    for players in 1..=4 {
        for second in [None, Some(901), Some(0)] {
            for (yes, no) in [(0x123400, 0x123500), (0, 0x123500), (0, 0)] {
                let scenario = Scenario::players(players).confirmation(900, second, yes, no);
                let original = translated(page.0, 0x2eb9e0, &scenario);
                let mut rec = ModelRecorder::new();
                Confirmation {
                    first: TextId(900),
                    second: second.map(TextId),
                    yes: (yes != 0).then_some(HandlerRef::Legacy(yes)),
                    no: (no != 0).then_some(HandlerRef::Legacy(no)),
                }
                .build(&mut rec, page)
                .unwrap();
                let native = rec.finish().unwrap();
                let diff = compare_pages(native.page(page).unwrap(), original.page(page).unwrap());
                assert!(
                    diff.exact(),
                    "players={players} second={second:?} yes={yes:#x} no={no:#x}: {diff:?}"
                );
            }
        }
    }
}

#[test]
fn native_save_status_matches_synthetic_dispatch_branches() {
    let page = PageId(0x4e8d84);
    // Branch targets deliberately assigned to fabricated states: this does not recover
    // the private image's state-to-branch table. It checks each known branch body.
    for (state, target, status) in [
        (0, 0x192142, SaveStatus::Bank(TextId(0xe68))),
        (1, 0x192162, SaveStatus::Runtime),
        (4, 0, SaveStatus::Empty),
    ] {
        let mut scenario = Scenario::single_player();
        scenario.words.extend([
            (0x74fa0c, 1),
            (0x74e940, state),
            (0x192194 + state * 4, target),
        ]);
        let original = translated(page.0, 0x192110, &scenario);
        let mut rec = ModelRecorder::new();
        status.build(&mut rec, page).unwrap();
        let native = rec.finish().unwrap();
        let diff = compare_pages(native.page(page).unwrap(), original.page(page).unwrap());
        assert!(diff.exact(), "state={state}: {diff:?}");
    }
}

#[test]
fn setup_rows_follow_mode_and_caller_table() {
    for mode in [0, 9, 11, 12, 13] {
        let scenario = Scenario::single_player()
            .unlocked()
            .setup_options(mode, &[(3, 901), (4, 902)]);
        let model = translated(0x523890, 0x2d6250, &scenario);
        let page = model.page(PageId(0x523890)).unwrap();
        assert_eq!(page.widgets.len(), 3, "two rows and a divider, mode={mode}");
        assert_eq!(page.widgets[0].id.0, 3);
        assert_eq!(page.widgets[0].text, TextRef::Id(TextId(901)));
        assert_eq!(page.widgets[2].id.0, 4);
        assert_eq!(page.widgets[2].text, TextRef::Id(TextId(902)));
        assert_eq!(page.widgets[0].activate, Some(HandlerRef::Legacy(0x2d6190)));
    }
}

#[test]
fn additional_catalogue_builders_work_without_an_image() {
    for (page, builder, rows) in [(0x524a80, 0x2ebe90, 4), (0x524ab8, 0x2ef6c0, 1)] {
        let model = translated(page, builder, &Scenario::single_player());
        assert_eq!(model.page(PageId(page)).unwrap().widgets.len(), rows);
    }
}

#[test]
fn table_boundaries_use_their_distinct_index_rules() {
    let mut memory = Ram::default();
    memory.write32(0x510788 + 20 * 4, 0x100100);
    memory.write32(0x513d04 + 21 * 4, 0x100200);
    memory.write32(0x513d04, 0xbad);
    assert_eq!(tables::descriptor(&memory, 20), 0x100100);
    assert_eq!(tables::descriptor(&memory, 21), 0x100200);
    assert_eq!(tables::record(6), 0x5107e0 + 6 * 0x38);
    assert_eq!(tables::record(7), 0x513b60);
    assert_eq!(tables::record(8), 0x513b60 + 0x38);
    assert_eq!(tables::profile(0), 0x7bedc4);
    assert_eq!(tables::profile(3), 0x7bedc4 + 3 * 0x1f54);
    memory.write32(0x4d28b8 + 2 * 4, 901);
    assert_eq!(tables::text_id(&memory, 2), 901);
}

#[test]
fn profile_leaf_agrees_with_translated_address_arithmetic() {
    use tsfp_ports::abi::{ExtRegisters, GuestCall};
    struct NoCalls;
    impl GuestCall for NoCalls {
        fn call(&mut self, target: u32, _: &mut ExtRegisters) {
            panic!("unexpected call {target:#x}");
        }
    }
    for slot in (0..4096).chain([u32::MAX, 0x8000_0000]) {
        let mut memory = Ram::default();
        let mut registers = tsfp_ui_builders::toolkit::regs();
        memory.write32(registers.esp + 4, slot);
        assert!(tsfp_ports::c2r::run(
            0x69620,
            &mut memory,
            &mut NoCalls,
            &mut registers
        ));
        assert_eq!(tables::profile(slot), registers.eax);
    }
}

#[test]
fn save_waiting_panel_uses_each_local_slot() {
    for count in 1..=4 {
        for slot in 0..count {
            let model = translated(
                0x4e8d54,
                0x194980,
                &Scenario::save_panel_waiting(count, slot),
            );
            let page = model.page(PageId(0x4e8d54)).unwrap();
            eprintln!(
                "save waiting players={count} slot={slot} rows={}",
                page.widgets.len()
            );
            assert!(
                page.widgets
                    .iter()
                    .any(|w| w.text == TextRef::Id(TextId(0x1992)))
            );
            assert_eq!(page.handlers.back, Some(HandlerRef::Legacy(0x1919a0)));
            assert!(page.widgets.iter().all(|w| !w.navigable));
        }
    }
}
