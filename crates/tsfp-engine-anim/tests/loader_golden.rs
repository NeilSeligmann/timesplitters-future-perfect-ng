//! T1668: `pak_list` (`0x1C8540`) against the ORIGINAL (`tools/t1668_loader_gold.py`).
use tsfp_engine_anim::loaders::pak_list;

#[test]
fn pak_list_equals_the_original() {
    let text = std::fs::read_to_string(format!("{}/tests/data/t1668/pak_list.golden", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let mut checked = 0;
    for line in text.lines() {
        let (head, list) = line.split_once(" : ").unwrap();
        let f: Vec<&str> = head.split(' ').collect();
        let want: Vec<&str> = if list.is_empty() { vec![] } else { list.split(',').collect() };
        let got = pak_list(f[1].parse().unwrap(), f[2].parse().unwrap(), u32::from_str_radix(f[3], 16).unwrap(), f[4].parse().unwrap());
        assert_eq!(got, want, "{line}");
        checked += 1;
    }
    assert_eq!(checked, 4160);
}
