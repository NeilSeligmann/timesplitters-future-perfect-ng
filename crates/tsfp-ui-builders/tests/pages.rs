//! Builds the front end pages with the ported builders and compares them with the dump trees
//! (ignored `tmp/ui-tree/tree.json` plus one tree per captured route in `tmp/ui-tree/variants/`,
//! or `TSFP_UI_TREE` for a single file). The comparison skips without a capture.
//!
//! Each page is built under several fixture scenarios (player count, unlocks, tables) and compared
//! with every captured variant of that page; the best pair is reported with its labels, so a page
//! counts as matched only when some captured game state is reproduced by some fixture state.

use std::collections::BTreeMap;
use tsfp_ui_builders::compare::{PageDiff, compare_pages};
use tsfp_ui_builders::image::find_image;
use tsfp_ui_builders::toolkit::{PageRun, Scenario, build_page};
use tsfp_ui_model::{LoadOptions, ModelRecorder, PageId, RectEdges, UiModel};

/// (page id = descriptor address, builder address)
const PAGES: &[(u64, u32, &str)] = &[
    (0x524598, 0x2d0be0, "main-menu"),
    (0x5246cc, 0x2d2350, "player-count"),
    (0x52377c, 0x2c3e20, "audio-video"),
    (0x524a40, 0x2ea8e0, "controls"),
    (0x524578, 0x2d08c0, "extras"),
];

/// Remaining pages of the dump whose builder is ported. Builders that read state the fixture
/// lacks (profile lists, level tables) are expected to differ; this is a survey, not a gate.
const SURVEY: &[(u64, u32)] = &[
    (0x5246e8, 0x2d7580),
    (0x4e8d54, 0x194980),
    (0x5238e0, 0x2cb090),
    (0x5238d0, 0x2d62f0),
    (0x5238a0, 0x2ca230),
    (0x5238c0, 0x2cac30),
    (0x523840, 0x2c8e00),
    (0x4e92c4, 0x1a2730),
    (0x523830, 0x2c8940),
    (0x523890, 0x2d6250),
    (0x523860, 0x2c9480),
    (0x4e9dd4, 0x1ad560),
    (0x523920, 0x2cc170),
    (0x5238b0, 0x2ca960),
    (0x523870, 0x2d5f40),
    (0x4e8d84, 0x192110),
    (0x523880, 0x2c9ae0),
    (0x523800, 0x2d5e70),
    (0x4e8e18, 0x1a1880),
    (0x5236dc, 0x2bf770),
    (0x5237f0, 0x2c6c80),
    (0x5237e0, 0x2c6680),
    (0x524700, 0x2f5520),
    (0x524740, 0x2f5e60),
    (0x524a70, 0x2eb9e0),
    (0x524750, 0x2f5920),
    (0x524730, 0x2f5170),
    (0x524710, 0x2f7570),
    (0x524a80, 0x2ebe90),
    (0x524ab8, 0x2ef6c0),
];

/// Fixture states with the static image mapped when it is available (private, `TSFP_XBE`).
fn scenarios() -> Vec<(&'static str, Scenario)> {
    static IMAGE: std::sync::OnceLock<Option<std::sync::Arc<tsfp_ui_builders::image::XbeImage>>> =
        std::sync::OnceLock::new();
    let image = IMAGE
        .get_or_init(|| find_image().map(std::sync::Arc::new))
        .clone();
    Scenario::all_with(image)
}

fn load_options() -> LoadOptions {
    LoadOptions {
        rect_edges: Some(RectEdges::DUMP),
        ignore_row_enabled_flag: true,
        ..Default::default()
    }
}

/// All captured trees: the merged tree plus the per-route variants (or the `TSFP_UI_TREE` file).
fn dumps() -> Vec<(String, UiModel)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp/ui-tree");
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(p) = std::env::var("TSFP_UI_TREE") {
        files.push(p.into());
    } else {
        files.push(root.join("tree.json"));
        if let Ok(dir) = std::fs::read_dir(root.join("variants")) {
            let mut v: Vec<_> = dir.flatten().map(|e| e.path()).collect();
            v.sort();
            files.extend(v);
        }
    }
    files
        .into_iter()
        .filter_map(|f| {
            let text = std::fs::read_to_string(&f).ok()?;
            let model = UiModel::from_tree_json(&text, &load_options()).ok()?;
            let name = f.file_stem()?.to_string_lossy().into_owned();
            Some((name, model))
        })
        .collect()
}

fn build(builder: u32, page: u64, scenario: &Scenario) -> Option<(UiModel, PageRun)> {
    // A watchdog thread turns a builder that loops on a fixture it cannot finish into a failed
    // build instead of a hung test (the thread is abandoned).
    let scenario = scenario.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(|| {
            let mut rec = ModelRecorder::new();
            let run = build_page(&mut rec, PageId(page), builder, &scenario);
            run.ok().and_then(|run| rec.finish().ok().map(|m| (m, run)))
        });
        let _ = tx.send(result.ok().flatten());
    });
    rx.recv_timeout(std::time::Duration::from_secs(5))
        .ok()
        .flatten()
}

/// Rank of a diff: fewer structural mismatches, then fewer text mismatches, then fewer missing rows.
fn rank(d: &PageDiff) -> (usize, usize, usize) {
    (
        d.structural_mismatches(),
        d.text_mismatch.len() + d.option_text_mismatch.len(),
        d.rows_dump.saturating_sub(d.rows_built),
    )
}

struct Best {
    scenario: &'static str,
    variant: String,
    diff: PageDiff,
    run: PageRun,
    built_pages: usize,
}

/// Best (scenario, variant) pair for a page, or None when no scenario builds it.
fn best_match(page: u64, builder: u32, dumps: &[(String, UiModel)]) -> Option<Best> {
    let mut best: Option<Best> = None;
    for (label, scenario) in scenarios() {
        let Some((model, run)) = build(builder, page, &scenario) else {
            continue;
        };
        let built = model.page(PageId(page)).unwrap();
        for (variant, dump) in dumps {
            let Some(d) = dump.page(PageId(page)) else {
                continue;
            };
            let diff = compare_pages(built, d);
            if best.as_ref().is_none_or(|b| rank(&diff) < rank(&b.diff)) {
                best = Some(Best {
                    scenario: label,
                    variant: variant.clone(),
                    diff,
                    run: PageRun {
                        unmodelled: run.unmodelled.clone(),
                        ignored: run.ignored.clone(),
                        missed: run.missed.clone(),
                        children: run.children,
                    },
                    built_pages: model.pages.len(),
                });
            }
        }
    }
    best
}

#[test]
fn builders_produce_pages_without_a_capture() {
    for (page, builder, name) in PAGES {
        let (model, run) = build(*builder, *page, &Scenario::single_player()).expect(name);
        let p = model.page(PageId(*page)).expect(name);
        assert!(!p.widgets.is_empty(), "{name}");
        eprintln!(
            "{name}: rows {} unmodelled {:x?}",
            p.widgets.len(),
            run.unmodelled
        );
    }
}

#[test]
fn compare_with_dump_trees() {
    let dumps = dumps();
    if dumps.is_empty() {
        eprintln!("no capture, skipped");
        return;
    }
    let (mut exact, mut structural, mut compared) = (0, 0, 0);
    for (page, builder, name) in PAGES {
        let Some(best) = best_match(*page, *builder, &dumps) else {
            eprintln!("{name}: not built or not in any capture");
            continue;
        };
        compared += 1;
        exact += usize::from(best.diff.exact());
        structural += usize::from(best.diff.structural_mismatches() == 0);
        eprintln!(
            "{name}: scenario {} variant {} exact={} structural={} {:?}",
            best.scenario,
            best.variant,
            best.diff.exact(),
            best.diff.structural_mismatches(),
            best.diff
        );
    }
    eprintln!("core pages compared {compared}, structural exact {structural}, fully exact {exact}");
    assert_eq!(compared, PAGES.len());
    assert_eq!(structural, PAGES.len(), "core pages are structurally exact");
}

/// Text ids differ only where the dump reports the lowest id of a bank string shared by several
/// ids (INFERRED alias rule) or where the fixture feeds a dynamic lookup. Alias pairs must be a
/// consistent function (the same built id always maps to the same dump id) and dump < built.
/// A built id below the dump id is a fixture state difference (reported, not an alias).
#[test]
fn text_mismatches_are_consistent_bank_aliases() {
    let dumps = dumps();
    if dumps.is_empty() {
        return;
    }
    let mut alias: BTreeMap<u32, u32> = Default::default();
    let mut state_differences = Vec::new();
    for (page, builder, name) in PAGES {
        let best = best_match(*page, *builder, &dumps).expect(name);
        assert_eq!(best.diff.structural_mismatches(), 0, "{name}");
        let pairs = best
            .diff
            .text_mismatch
            .iter()
            .map(|t| (t.1, t.2))
            .chain(best.diff.option_text_mismatch.iter().map(|t| (t.2, t.3)));
        for (built, dumped) in pairs {
            let (Some(b), Some(d)) = (built, dumped) else {
                continue;
            };
            if b == 0 {
                continue;
            } // id 0 is the fixture's empty table lookup
            if d > b {
                // The image tables now feed profile dependent texts (the controls preset name):
                // a different fixture state, not an alias of the same string.
                state_differences.push((name, b, d));
                continue;
            }
            assert_eq!(
                *alias.entry(b).or_insert(d),
                d,
                "{name}: inconsistent alias for {b}"
            );
        }
    }
    eprintln!("alias pairs built->dump: {alias:?}");
    eprintln!("fixture state differences (page, built id, dump id): {state_differences:?}");
}

#[test]
fn survey_all_ported_builders() {
    let dumps = dumps();
    if dumps.is_empty() {
        eprintln!("no capture, skipped");
        return;
    }
    let all: Vec<(u64, u32)> = PAGES
        .iter()
        .map(|(p, b, _)| (*p, *b))
        .chain(SURVEY.iter().copied())
        .collect();
    let (mut built, mut failed, mut exact, mut structural_exact) = (0, 0, 0, 0);
    let (mut rows_built, mut rows_dump, mut rows_ok) = (0usize, 0usize, 0usize);
    let mut pages_in_dump = 0;
    for (page, builder) in all {
        if !dumps.iter().any(|(_, d)| d.page(PageId(page)).is_some()) {
            continue;
        }
        pages_in_dump += 1;
        let Some(best) = best_match(page, builder, &dumps) else {
            failed += 1;
            eprintln!("{page:#x} builder {builder:#x}: build failed under every scenario");
            continue;
        };
        built += 1;
        let diff = &best.diff;
        exact += usize::from(diff.exact());
        structural_exact += usize::from(diff.structural_mismatches() == 0);
        // Rows compared against the best variant: every dump row counts as wanted.
        rows_built += diff.rows_built.min(diff.rows_dump);
        rows_dump += diff.rows_dump;
        let bad: std::collections::BTreeSet<usize> = diff
            .id_mismatch
            .iter()
            .chain(&diff.kind_mismatch)
            .chain(&diff.navigable_mismatch)
            .chain(&diff.activate_mismatch)
            .chain(&diff.option_count_mismatch)
            .copied()
            .chain(diff.text_mismatch.iter().map(|t| t.0))
            .chain(diff.option_text_mismatch.iter().map(|t| t.0))
            .collect();
        rows_ok += diff.rows_compared - bad.len();
        let missed: Vec<String> = best
            .run
            .missed
            .keys()
            .filter(|a| !(0x7f000..0x80000).contains(*a) && **a < 0x7000_0000)
            .take(6)
            .map(|a| format!("{a:#x}"))
            .collect();
        eprintln!(
            "{page:#x} builder {builder:#x}: scenario {} variant {} rows {}/{} structural {} text {} option-text {} unmodelled {:x?} missed {:?}",
            best.scenario,
            best.variant,
            diff.rows_built,
            diff.rows_dump,
            diff.structural_mismatches(),
            diff.text_mismatch.len(),
            diff.option_text_mismatch.len(),
            best.run.unmodelled.keys().collect::<Vec<_>>(),
            missed
        );
        let _ = best.built_pages;
    }
    eprintln!(
        "SURVEY pages {pages_in_dump} built {built} failed {failed} exact {exact} structural-exact {structural_exact} rows built {rows_built} dump {rows_dump} rows fully equal {rows_ok}"
    );
}

/// Detailed row listing of one page under every scenario (`TSFP_PAGE=0x523800`, ids only).
#[test]
fn inspect_page() {
    let Ok(want) = std::env::var("TSFP_PAGE") else {
        return;
    };
    let page = u64::from_str_radix(want.trim_start_matches("0x"), 16).unwrap();
    let builder = PAGES
        .iter()
        .map(|(p, b, _)| (*p, *b))
        .chain(SURVEY.iter().copied())
        .find(|(p, _)| *p == page)
        .expect("known page")
        .1;
    let dumps = dumps();
    let Some(best) = best_match(page, builder, &dumps) else {
        eprintln!("no scenario builds it");
        return;
    };
    eprintln!("best scenario {} variant {}", best.scenario, best.variant);
    let (_, scenario) = scenarios()
        .into_iter()
        .find(|(l, _)| *l == best.scenario)
        .unwrap();
    let (model, run) = build(builder, page, &scenario).unwrap();
    let built = model.page(PageId(page)).unwrap();
    let dump = dumps
        .iter()
        .find(|(n, _)| *n == best.variant)
        .and_then(|(_, d)| d.page(PageId(page)))
        .unwrap();
    for i in 0..built.widgets.len().max(dump.widgets.len()) {
        let f = |w: Option<&tsfp_ui_model::Widget>| {
            w.map_or("-".to_string(), |w| {
                format!(
                    "{:?} id {:#x} {:?} nav={} opts={} act={:?}",
                    w.kind,
                    w.id.0,
                    w.text,
                    w.navigable,
                    w.options.len(),
                    w.activate
                )
            })
        };
        eprintln!(
            "{i:2} built {} | dump {}",
            f(built.widgets.get(i)),
            f(dump.widgets.get(i))
        );
    }
    eprintln!("unmodelled {:x?}", run.unmodelled);
    eprintln!("ignored {:x?}", run.ignored);
    eprintln!("missed {:x?}", run.missed.keys().collect::<Vec<_>>());
}

/// A separate host-free count: never describe this as capture structural parity.
#[test]
fn survey_synthetic_builder_availability() {
    let all = PAGES
        .iter()
        .map(|(p, b, _)| (*p, *b))
        .chain(SURVEY.iter().copied());
    let mut built = 0;
    let mut failed = Vec::new();
    for (page, builder) in all {
        if Scenario::all()
            .iter()
            .any(|(_, s)| build(builder, page, s).is_some())
        {
            built += 1;
        } else {
            failed.push((page, builder));
        }
    }
    eprintln!("SYNTHETIC survey built {built}/35 failed {failed:x?}");
    assert_eq!(built, 34);
    assert_eq!(failed, vec![(0x5236dc, 0x2bf770)]);
}
