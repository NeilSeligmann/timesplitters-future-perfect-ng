// SPDX-License-Identifier: GPL-3.0-or-later
//! Replays `nav_golden.tsv` (the C nav grammar's output, regenerated and checked by
//! tests/test_t3975_nav_spec_golden.py) through the Rust parser, and pins the repr(C) layouts to `layout.tsv`.
use std::mem::{offset_of, size_of};
use tsfp_input_route_spec::cond::{self, Cond};
use tsfp_input_route_spec::nav_spec::{
    self as nav, Menu, MenuTable, Names, Ref, Step, Widgets, c_text,
};
use tsfp_input_route_spec::wait::Wait;

fn cond_text(c: &Cond) -> String {
    format!(
        "[{}|{}|{:x}|{:x}|{}|{}|{:x}|{:x}|{}|{:x}|{}|{}]",
        c.kind,
        u8::from(c.indirect),
        c.address,
        c.offset,
        c.width,
        c.cmp,
        c.value,
        c.mask,
        String::from_utf8_lossy(c.text_bytes()),
        c.va,
        c.amount,
        cond::format(c)
    )
}

fn ref_text(r: &Ref) -> String {
    format!("R{}{}", u8::from(r.page), cond_text(&r.cond))
}

fn wait_text(w: &Wait) -> String {
    let mut line = format!(
        "W mark={} any={} event={} minp={} minms={} maxp={} tmo={} mem={},{:x},{},{:x},{:x} conds={}",
        w.mark,
        u8::from(w.any),
        u8::from(w.event),
        w.min_polls,
        w.min_ms,
        w.max_polls,
        w.timeout_ms,
        u8::from(w.has_mem),
        w.address,
        w.width,
        w.value,
        w.mask,
        w.cond_count
    );
    for c in &w.conds[..w.cond_count as usize] {
        line.push(' ');
        line.push_str(&cond_text(c));
    }
    line
}

fn s(bytes: &[u8]) -> String {
    String::from_utf8_lossy(c_text(bytes)).into_owned()
}

fn step_text(st: &Step) -> String {
    let mut line = format!(
        "OK at={} to={} menu={} kind={} index={} name={} sel={} act={} hx={} expect={} tmo={} retry={} ",
        st.at,
        st.to,
        s(&st.menu),
        st.kind,
        st.index,
        s(&st.name),
        s(&st.select_text),
        u8::from(st.activate),
        u8::from(st.has_expect),
        s(&st.expect),
        st.timeout_ms,
        st.retry
    );
    if st.has_expect {
        line.push_str(&wait_text(&st.expect_wait));
    }
    line
}

fn widgets_text(w: &Widgets) -> String {
    let mut line = format!(
        "widgets={},{:x},{:x},{:x},{},",
        u8::from(w.present),
        w.table,
        w.stride,
        w.count,
        w.closing_count
    );
    for state in &w.closing[..w.closing_count as usize] {
        line.push_str(&format!("{state:x};"));
    }
    line
}

fn menu_text(m: &Menu) -> String {
    let mut line = format!(
        "id={} dpad_off={} by={} pb={} builder={:x} {}",
        s(&m.id),
        u8::from(m.dpad_off),
        m.selectby,
        u8::from(m.page_builder),
        m.builder,
        widgets_text(&m.widgets)
    );
    line.push_str(&format!(" active={}:", m.active_count));
    for r in &m.active[..m.active_count as usize] {
        line.push_str(&ref_text(r));
    }
    line.push_str(&format!(" ready={}:", m.ready_count));
    for r in &m.ready[..m.ready_count as usize] {
        line.push_str(&ref_text(r));
    }
    line.push_str(&format!(
        " hc={} cursor={} count={} itemid={}:{} vert={} wrap={} items={},{}",
        u8::from(m.has_cursor),
        ref_text(&m.cursor),
        ref_text(&m.count),
        u8::from(m.has_itemid),
        ref_text(&m.itemid),
        u8::from(m.vertical),
        u8::from(m.wrap),
        u8::from(m.items.present),
        ref_text(&m.items.head)
    ));
    let it = &m.items;
    line.push_str(&format!(
        ",{:x},{:x},{:x},{:x},{:x},{}:",
        it.next_off, it.type_off, it.id_off, it.text_off, it.flags_off, it.type_count
    ));
    for t in &it.types[..it.type_count as usize] {
        line.push_str(&format!("{t:x};"));
    }
    let names: &Names = &m.names;
    line.push_str(&format!(
        ",{:x},{:x} names={},{}",
        it.skip_mask,
        it.grey_mask,
        names.kind,
        ref_text(&names.base)
    ));
    line.push_str(&format!(
        ",{},{} sel={},{:x},{} back={} conf={} line={}",
        names.stride,
        names.max_len,
        m.select_analog,
        m.select_digital,
        s(&m.select_name),
        s(&m.back_name),
        s(&m.conf),
        m.line
    ));
    line
}

fn unescape(spec: &str) -> Vec<u8> {
    spec.replace("\\n", "\n").into_bytes()
}

fn run(mode: &str, spec: &str) -> String {
    match mode {
        "nav" => match nav::line_parse(Some(spec.as_bytes())) {
            Err(message) => format!("ERR {message}"),
            Ok(step) => step_text(&step),
        },
        "menus" => match nav::menus_parse(&unescape(spec)) {
            Err(message) => format!("ERR {message}"),
            Ok(table) => {
                let mut line = format!("OK count={} {}", table.count, widgets_text(&table.widgets));
                for menu in &table.menus[..table.count] {
                    line.push_str(" ;; ");
                    line.push_str(&menu_text(menu));
                }
                line
            }
        },
        "timing" => match nav::timing_parse(Some(spec.as_bytes())) {
            Err(message) => format!("ERR {message}"),
            Ok(t) => format!("OK {}:{}:{}", t.hold, t.gap, t.extra),
        },
        "mode" => {
            nav::mode_parse(Some(spec.as_bytes())).map_or("ERR".to_string(), |m| format!("OK {m}"))
        }
        "pct" => match nav::percent_decode(spec.as_bytes(), spec.len(), 64) {
            Err(message) => format!("ERR {message}"),
            Ok(name) => format!("OK {}", String::from_utf8_lossy(&name)),
        },
        "steps" => {
            let mut parts = spec.splitn(3, '|');
            let (total, marks, lines) =
                (parts.next().unwrap(), parts.next().unwrap(), parts.next());
            let Some(lines) = lines else {
                return "ERR shape".to_string();
            };
            let total: u64 = total.parse().unwrap();
            let marks: Vec<u64> = marks
                .split(',')
                .filter(|m| !m.is_empty())
                .map(|m| m.parse().unwrap())
                .collect();
            let mut steps = Vec::new();
            let mut rest = lines;
            while !rest.is_empty() && steps.len() < 70 {
                let (head, tail) = rest
                    .split_once(';')
                    .map_or((rest, None), |(h, t)| (h, Some(t)));
                match nav::line_parse(Some(head.as_bytes())) {
                    Err(message) => return format!("ERR parse {message}"),
                    Ok(step) => steps.push(step),
                }
                match tail {
                    None => break,
                    Some(t) => rest = t,
                }
            }
            match nav::steps_check(&steps, &marks, total) {
                Err(message) => format!("ERR {message}"),
                Ok(()) => format!("OK {}", steps.len()),
            }
        }
        other => panic!("unknown mode {other}"),
    }
}

#[test]
fn rust_nav_parser_equals_the_c_golden() {
    let golden = include_str!("nav_golden.tsv");
    let mut checked = 0;
    let mut failures = Vec::new();
    for line in golden.lines() {
        let mut parts = line.splitn(3, '\t');
        let (mode, spec, expected) = (
            parts.next().unwrap(),
            parts.next().unwrap(),
            parts.next().unwrap(),
        );
        let got = run(mode, spec);
        checked += 1;
        if got != expected {
            failures.push(format!("{mode} {spec:?}\n  want {expected}\n  got  {got}"));
        }
    }
    assert!(checked > 6000, "golden has {checked} lines");
    assert!(
        failures.is_empty(),
        "{} of {checked} differ, first:\n{}",
        failures.len(),
        failures[..failures.len().min(5)].join("\n")
    );
}

#[test]
fn repr_c_layouts_equal_the_c_headers() {
    let layout = include_str!("layout.tsv");
    let mut checked = 0;
    for line in layout.lines() {
        let mut parts = line.split(' ');
        let (kind, name, want) = (
            parts.next().unwrap(),
            parts.next().unwrap(),
            parts.next().unwrap(),
        );
        let want: usize = want.parse().unwrap();
        let got = match (kind, name) {
            ("sizeof", "route_cond") => size_of::<Cond>(),
            ("sizeof", "xinput_route_wait") => size_of::<Wait>(),
            ("sizeof", "route_nav_timing") => size_of::<nav::Timing>(),
            ("sizeof", "route_nav_step") => size_of::<Step>(),
            ("sizeof", "route_nav_ref") => size_of::<Ref>(),
            ("sizeof", "route_nav_names") => size_of::<Names>(),
            ("sizeof", "route_nav_items") => size_of::<nav::Items>(),
            ("sizeof", "route_nav_widgets") => size_of::<Widgets>(),
            ("sizeof", "route_nav_menu") => size_of::<Menu>(),
            ("sizeof", "route_nav_menu_table") => size_of::<MenuTable>(),
            ("offsetof", "route_nav_step.kind") => offset_of!(Step, kind),
            ("offsetof", "route_nav_step.name") => offset_of!(Step, name),
            ("offsetof", "route_nav_step.select_text") => offset_of!(Step, select_text),
            ("offsetof", "route_nav_step.activate") => offset_of!(Step, activate),
            ("offsetof", "route_nav_step.expect") => offset_of!(Step, expect),
            ("offsetof", "route_nav_step.expect_wait") => offset_of!(Step, expect_wait),
            ("offsetof", "route_nav_step.timeout_ms") => offset_of!(Step, timeout_ms),
            ("offsetof", "route_nav_step.retry") => offset_of!(Step, retry),
            ("offsetof", "route_nav_names.base") => offset_of!(Names, base),
            ("offsetof", "route_nav_names.stride") => offset_of!(Names, stride),
            ("offsetof", "route_nav_items.head") => offset_of!(nav::Items, head),
            ("offsetof", "route_nav_items.types") => offset_of!(nav::Items, types),
            ("offsetof", "route_nav_items.skip_mask") => offset_of!(nav::Items, skip_mask),
            ("offsetof", "route_nav_widgets.closing") => offset_of!(Widgets, closing),
            ("offsetof", "route_nav_widgets.closing_count") => offset_of!(Widgets, closing_count),
            ("offsetof", "route_nav_menu.dpad_off") => offset_of!(Menu, dpad_off),
            ("offsetof", "route_nav_menu.selectby") => offset_of!(Menu, selectby),
            ("offsetof", "route_nav_menu.builder") => offset_of!(Menu, builder),
            ("offsetof", "route_nav_menu.widgets") => offset_of!(Menu, widgets),
            ("offsetof", "route_nav_menu.active") => offset_of!(Menu, active),
            ("offsetof", "route_nav_menu.ready") => offset_of!(Menu, ready),
            ("offsetof", "route_nav_menu.cursor") => offset_of!(Menu, cursor),
            ("offsetof", "route_nav_menu.has_itemid") => offset_of!(Menu, has_itemid),
            ("offsetof", "route_nav_menu.items") => offset_of!(Menu, items),
            ("offsetof", "route_nav_menu.names") => offset_of!(Menu, names),
            ("offsetof", "route_nav_menu.select_analog") => offset_of!(Menu, select_analog),
            ("offsetof", "route_nav_menu.select_digital") => offset_of!(Menu, select_digital),
            ("offsetof", "route_nav_menu.select_name") => offset_of!(Menu, select_name),
            ("offsetof", "route_nav_menu.conf") => offset_of!(Menu, conf),
            ("offsetof", "route_nav_menu.line") => offset_of!(Menu, line),
            ("offsetof", "route_nav_menu_table.menus") => offset_of!(MenuTable, menus),
            ("offsetof", "route_nav_menu_table.count") => offset_of!(MenuTable, count),
            other => panic!("no Rust counterpart for {other:?}"),
        };
        assert_eq!(got, want, "{kind} {name}");
        checked += 1;
    }
    assert!(checked > 40, "layout has {checked} lines");
}
