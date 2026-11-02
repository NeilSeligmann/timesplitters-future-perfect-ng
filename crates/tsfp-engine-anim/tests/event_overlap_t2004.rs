//! T2004 Tier 1: `0x61450` event flag overlap against numbers recorded from the ORIGINAL under Unicorn
//! (`tools/t2004_event_overlap_gold.py`, resident lookup 0x56FC0 stubbed).
use tsfp_engine_anim::queries::{event_flags_overlap, Event};

#[test]
fn event_flags_overlap_equals_the_original() {
    let text = include_str!("data/t2004/event_overlap.golden");
    let (mut checked, mut yes, mut lookups) = (0, 0, 0);
    for line in text.lines() {
        let f: Vec<&str> = line.split(' ').collect();
        let id = u32::from_str_radix(f[1], 16).unwrap() as i32;
        let mask = u32::from_str_radix(f[2], 16).unwrap();
        let want = u32::from_str_radix(f[3], 16).unwrap() != 0;
        let looked_up = f[4] == "1";
        let list: Vec<Event> = if f[6] == "-" {
            Vec::new()
        } else {
            f[6].split(',').map(|v| Event { frame: 0, flags: u16::from_str_radix(v, 16).unwrap() }).collect()
        };
        // the original only looks the animation up for an id above 0
        assert_eq!(id > 0, looked_up, "{line}");
        let events = (!f[5].starts_with('n')).then_some(list.as_slice());
        assert_eq!(event_flags_overlap(id, events, mask), want, "{line}");
        checked += 1;
        yes += usize::from(want);
        lookups += usize::from(looked_up);
    }
    assert!(checked >= 2000 && yes > 300 && lookups > 1000, "{checked} {yes} {lookups}");
}
