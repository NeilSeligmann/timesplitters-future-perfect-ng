//! Spec tests for the enumeration cursor (T2074). FABRICATED inputs, Tier 0/1.
use tsfp_engine_fileio::dirpick::{Pick, mask_matches, name_order, pick};

fn run(names: &[&str], mask: &str, last: Option<&str>, max: usize) -> Pick {
    pick(names.iter().map(|n| n.as_bytes()), mask.as_bytes(), last.map(str::as_bytes), max)
}

#[test]
fn masks() {
    assert!(mask_matches(b"", b"anything"));
    assert!(mask_matches(b"*.*", b"no_dot"));
    assert!(mask_matches(b"*.XBE", b"default.xbe"));
    assert!(mask_matches(b"a?c*", b"ABCDE"));
    assert!(!mask_matches(b"a?c", b"abcd"));
    assert!(!mask_matches(b"*.xbe", b"default.xb"));
}

#[test]
fn order_is_folded_then_bytes() {
    use std::cmp::Ordering::*;
    assert_eq!(name_order(b"B", b"a"), Greater);
    assert_eq!(name_order(b"a", b"A"), Greater); // tie broken by byte value, 'a' (97) > 'A' (65)
    assert_eq!(name_order(b"ab", b"abc"), Less);
}

#[test]
fn walks_in_order_with_cursor() {
    let names = ["b", "C", "a", "d"];
    let mut last: Option<&str> = None;
    let mut seen = Vec::new();
    while let Pick::Found(i) = run(&names, "", last, 255) {
        seen.push(names[i]);
        last = Some(names[i]);
    }
    assert_eq!(seen, ["a", "b", "C", "d"]);
    assert_eq!(run(&names, "", last, 255), Pick::None);
}

#[test]
fn too_long_does_not_hide_it() {
    assert_eq!(run(&["abcdef"], "", None, 3), Pick::TooLong);
    assert_eq!(run(&["abcdef"], "", None, 6), Pick::Found(0));
}
