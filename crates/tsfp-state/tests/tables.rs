// SPDX-License-Identifier: GPL-3.0-or-later
// FABRICATED synthetic XBE (patterned bytes). The optional retail check reads TSFP_XBE and prints
// nothing but pass or fail; without the variable it is skipped (and says so).
use tsfp_state::GuestMemory;
use tsfp_state::generated_tables::TABLES;
use tsfp_state::generated_views::TsfpWeaponRecordView;
use tsfp_state::sha256::{matches, sha256};
use tsfp_state::tables::{TableError, TableSpec, Tables};
use tsfp_state::xbe::{XbeError, XbeImage};

fn hex(digest: [u8; 32]) -> String {
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn sha256_known_vectors() {
    assert_eq!(
        hex(sha256(b"")),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        hex(sha256(b"abc")),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    let long = [b'a'; 1000];
    assert_eq!(
        hex(sha256(&long)),
        "41edece42d63e8d9bf515a9ba6932e1c20cbc9f5a5d134645adb5db1b9737ea3"
    );
    assert!(matches(
        b"abc",
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    ));
    assert!(!matches(
        b"abd",
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    ));
}

#[test]
fn sha256_padding_boundaries_and_strict_hex_matching() {
    // Independent hashlib vectors straddle both the 56-byte padding cutoff and block edges.
    const CASES: &[(usize, &str)] = &[
        (
            55,
            "463eb28e72f82e0a96c0a4cc53690c571281131f672aa229e0d45ae59b598b59",
        ),
        (
            56,
            "da2ae4d6b36748f2a318f23e7ab1dfdf45acdc9d049bd80e59de82a60895f562",
        ),
        (
            63,
            "29af2686fd53374a36b0846694cc342177e428d1647515f078784d69cdb9e488",
        ),
        (
            64,
            "fdeab9acf3710362bd2658cdc9a29e8f9c757fcf9811603a8c447cd1d9151108",
        ),
        (
            65,
            "4bfd2c8b6f1eec7a2afeb48b934ee4b2694182027e6d0fc075074f2fabb31781",
        ),
        (
            119,
            "da18797ed7c3a777f0847f429724a2d8cd5138e6ed2895c3fa1a6d39d18f7ec6",
        ),
        (
            120,
            "f52b23db1fbb6ded89ef42a23ce0c8922c45f25c50b568a93bf1c075420bbb7c",
        ),
    ];
    for &(len, expected) in CASES {
        let data: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
        assert_eq!(hex(sha256(&data)), expected, "length {len}");
        assert!(matches(&data, expected));
        assert!(matches(&data, &expected.to_ascii_uppercase()));
        assert!(!matches(&data, &expected[..63]));
        let mut wrong = expected.as_bytes().to_vec();
        wrong[0] = b'g';
        assert!(!matches(&data, core::str::from_utf8(&wrong).unwrap()));
    }
}

const BASE: u32 = 0x10000;
const SECTION_VA: u32 = 0x4b8000;
const RAW: usize = 0x1000;

/// Minimal XBE: header, one section header, one file-backed section (0x1000 bytes) inside a
/// larger virtual size (the tail is zero filled at run time, not in the file).
fn synthetic_xbe() -> Vec<u8> {
    let mut x = vec![0u8; RAW + 0x1000];
    x[0..4].copy_from_slice(b"XBEH");
    x[0x104..0x108].copy_from_slice(&BASE.to_le_bytes());
    x[0x11c..0x120].copy_from_slice(&1u32.to_le_bytes());
    x[0x120..0x124].copy_from_slice(&(BASE + 0x200).to_le_bytes());
    let h = 0x200;
    x[h + 4..h + 8].copy_from_slice(&SECTION_VA.to_le_bytes());
    x[h + 8..h + 12].copy_from_slice(&0x4000u32.to_le_bytes());
    x[h + 12..h + 16].copy_from_slice(&(RAW as u32).to_le_bytes());
    x[h + 16..h + 20].copy_from_slice(&0x1000u32.to_le_bytes());
    for i in 0..0x1000 {
        x[RAW + i] = (i * 13 + 5) as u8;
    }
    x
}

fn specs(xbe: &[u8], corrupt: bool, tail: bool) -> &'static [TableSpec] {
    let leak = |s: String| -> &'static str { Box::leak(s.into_boxed_str()) };
    let body = &xbe[RAW + 0x100..RAW + 0x100 + 0x84];
    let mut digest = hex(sha256(body));
    if corrupt {
        digest.replace_range(0..1, if digest.starts_with('0') { "1" } else { "0" });
    }
    let first = TableSpec {
        name: "weapon_records",
        va: SECTION_VA + 0x100,
        size: 0x84,
        stride: 0x42,
        count: 2,
        sha256: leak(digest),
    };
    let second_va = if tail {
        SECTION_VA + 0x1800
    } else {
        SECTION_VA + 0x400
    };
    let second = TableSpec {
        name: "mode_rows",
        va: second_va,
        size: 0x20,
        stride: 0x10,
        count: 2,
        sha256: leak(hex(sha256(&xbe[RAW + 0x400..RAW + 0x420]))),
    };
    Box::leak(vec![first, second].into_boxed_slice())
}

#[test]
fn loads_tables_and_reads_them_through_typed_views() {
    let xbe = synthetic_xbe();
    let tables = Tables::from_xbe_with(&xbe, specs(&xbe, false, false), true).unwrap();
    assert_eq!(
        tables.body("weapon_records").unwrap(),
        &xbe[RAW + 0x100..RAW + 0x184]
    );
    assert!(tables.body("nope").is_none());
    // address arithmetic: record 1 starts one stride after the table base
    let record = tables.weapon_record(1).unwrap();
    assert_eq!(record, TsfpWeaponRecordView(SECTION_VA + 0x100 + 0x42));
    assert!(tables.weapon_record(2).is_none());
    // the first dword of record 0 equals the file bytes (little endian)
    assert_eq!(
        tables.read32(SECTION_VA + 0x100),
        u32::from_le_bytes(xbe[RAW + 0x100..RAW + 0x104].try_into().unwrap())
    );
}

#[test]
fn rejects_wrong_hash_unbacked_tail_and_bad_images() {
    let xbe = synthetic_xbe();
    assert_eq!(
        Tables::from_xbe_with(&xbe, specs(&xbe, true, false), true).err(),
        Some(TableError::HashMismatch("weapon_records"))
    );
    assert!(
        Tables::from_xbe_with(&xbe, specs(&xbe, true, false), false).is_ok(),
        "verify off skips the hash"
    );
    // the second table sits in the zero filled tail of the section: no bytes in the file
    assert_eq!(
        Tables::from_xbe_with(&xbe, specs(&xbe, false, true), true).err(),
        Some(TableError::NotFileBacked("mode_rows"))
    );
    assert_eq!(XbeImage::parse(b"NOPE").err(), Some(XbeError::BadMagic));
    assert_eq!(XbeImage::parse(b"XBEH").err(), Some(XbeError::Truncated));
}

struct Image(Vec<u8>);
impl GuestMemory for Image {
    fn read8(&self, a: u32) -> u8 {
        self.0[(a - SECTION_VA) as usize]
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0[(a - SECTION_VA) as usize] = v;
    }
}

#[test]
fn encode_into_writes_exactly_the_table_bytes() {
    let xbe = synthetic_xbe();
    let tables = Tables::from_xbe_with(&xbe, specs(&xbe, false, false), true).unwrap();
    let mut image = Image(vec![0xee; 0x1000]);
    tables.encode_into(&mut image);
    for spec in tables.specs() {
        let at = (spec.va - SECTION_VA) as usize;
        assert_eq!(
            &image.0[at..at + spec.size as usize],
            tables.body(spec.name).unwrap()
        );
    }
    let written = image.0.iter().filter(|b| **b != 0xee).count();
    assert!(
        written > 0x80,
        "bytes outside the tables stay untouched ({written} changed)"
    );
    assert_eq!(image.0[0], 0xee);
}

#[test]
#[should_panic(expected = "immutable")]
fn writing_an_immutable_table_panics() {
    let xbe = synthetic_xbe();
    let mut tables = Tables::from_xbe_with(&xbe, specs(&xbe, false, false), true).unwrap();
    tables.write8(SECTION_VA + 0x100, 1);
}

#[test]
#[should_panic(expected = "outside the immutable tables")]
fn reading_outside_the_tables_panics() {
    let xbe = synthetic_xbe();
    let tables = Tables::from_xbe_with(&xbe, specs(&xbe, false, false), true).unwrap();
    tables.read8(SECTION_VA);
}

#[test]
fn pinned_tables_match_the_retail_xbe_when_provided() {
    let Some(path) = std::env::var_os("TSFP_XBE") else {
        eprintln!("skipped: TSFP_XBE not set (retail check)");
        return;
    };
    let xbe = std::fs::read(path).unwrap();
    let tables = Tables::from_xbe(&xbe).expect("pinned hashes match the retail XBE");
    assert_eq!(tables.specs().len(), TABLES.len());
    for spec in TABLES {
        assert_eq!(spec.stride * spec.count, spec.size, "{}", spec.name);
        assert_eq!(tables.body(spec.name).unwrap().len() as u32, spec.size);
    }
}

#[test]
fn owned_tables_survive_source_drop_and_preserve_mapping() {
    use tsfp_state::tables::OwnedTables;
    let xbe = synthetic_xbe();
    let expected = xbe[RAW + 0x100..RAW + 0x184].to_vec();
    let owned = OwnedTables::from_xbe_with(&xbe, specs(&xbe, false, false), true).unwrap();
    drop(xbe);
    assert_eq!(owned.body("weapon_records").unwrap(), expected);
    let mut image = Image(vec![0xee; 0x1000]);
    owned.mirror_into(&mut image);
    for spec in owned.as_tables().specs() {
        for i in 0..spec.size {
            assert_eq!(owned.read8(spec.va + i), image.read8(spec.va + i));
        }
    }
    for i in 0..0x1000 {
        let address = SECTION_VA + i;
        if !owned
            .as_tables()
            .specs()
            .iter()
            .any(|s| address >= s.va && address - s.va < s.size)
        {
            assert_eq!(image.read8(address), 0xee);
        }
    }
}

#[test]
fn rejects_overlapping_and_overflowing_specs() {
    let xbe = synthetic_xbe();
    for kind in 0..4 {
        let mut bad = specs(&xbe, false, false).to_vec();
        match kind {
            0 => bad[1].va = bad[0].va,
            1 => bad[0].va = u32::MAX,
            2 => bad[0].count = u32::MAX,
            _ => bad[1].name = bad[0].name,
        }
        assert_eq!(
            Tables::from_xbe_with(&xbe, Box::leak(bad.into_boxed_slice()), false).err(),
            Some(TableError::InvalidSpec)
        );
    }
}

#[test]
fn disc_loader_uses_synthetic_xdvdfs_and_owns_the_result() {
    use tsfp_engine_fileio::xiso::{MAGIC, SECTOR, Xiso};
    use tsfp_state::tables::OwnedTables;
    let xbe = synthetic_xbe();
    let mut iso = vec![0u8; 40 * SECTOR as usize];
    let d = 32 * SECTOR as usize;
    iso[d..d + 20].copy_from_slice(MAGIC);
    iso[d + 0x7ec..d + 0x800].copy_from_slice(MAGIC);
    iso[d + 0x14..d + 0x18].copy_from_slice(&33u32.to_le_bytes());
    iso[d + 0x18..d + 0x1c].copy_from_slice(&25u32.to_le_bytes());
    let dir = 33 * SECTOR as usize;
    iso[dir + 4..dir + 8].copy_from_slice(&34u32.to_le_bytes());
    iso[dir + 8..dir + 12].copy_from_slice(&(xbe.len() as u32).to_le_bytes());
    iso[dir + 13] = 11;
    iso[dir + 14..dir + 25].copy_from_slice(b"default.xbe");
    let at = 34 * SECTOR as usize;
    iso[at..at + xbe.len()].copy_from_slice(&xbe);
    let path = std::env::temp_dir().join(format!("tsfp-t2059-{}.iso", std::process::id()));
    std::fs::write(&path, iso).unwrap();
    let disc = Xiso::open(&path).unwrap();
    let entry = disc.lookup("default.xbe").unwrap();
    let mut bytes = vec![0; entry.size as usize];
    assert_eq!(disc.read(&entry, 0, &mut bytes).unwrap(), bytes.len());
    let owned = OwnedTables::from_xbe_with(&bytes, specs(&xbe, false, false), true).unwrap();
    drop(disc);
    std::fs::remove_file(path).unwrap();
    assert_eq!(
        owned.body("weapon_records").unwrap(),
        &xbe[RAW + 0x100..RAW + 0x184]
    );
}

#[test]
fn owned_weapon_and_mode_fields_use_recovered_layouts() {
    use tsfp_state::tables::OwnedTables;
    let mut xbe = synthetic_xbe();
    let weapon = RAW + 0x100;
    xbe[weapon + 0x18..weapon + 0x1c].copy_from_slice(&17u32.to_le_bytes());
    let mode = RAW + 0x800;
    xbe[mode + 4..mode + 8].copy_from_slice(&23u32.to_le_bytes());
    let specs = Box::leak(
        vec![
            TableSpec {
                name: "weapon_records",
                va: SECTION_VA + 0x100,
                size: 0x538,
                stride: 0x29c,
                count: 2,
                sha256: "",
            },
            TableSpec {
                name: "mode_rows",
                va: SECTION_VA + 0x800,
                size: 0x100,
                stride: 0x80,
                count: 2,
                sha256: "",
            },
        ]
        .into_boxed_slice(),
    );
    let owned = OwnedTables::from_xbe_with(&xbe, specs, false).unwrap();
    drop(xbe);
    assert_eq!(owned.weapon_record(0).unwrap().clip_size_1, 17);
    assert_eq!(owned.mode_row(0).unwrap().mode_id, 23);
    assert!(owned.weapon_record(2).is_none());
    assert!(owned.mode_row(2).is_none());
}

#[test]
fn complete_weapon_and_mode_counts_are_mapped_without_relocation() {
    use tsfp_state::tables::OwnedTables;
    let mut xbe = synthetic_xbe();
    xbe.resize(RAW + 0x20000, 0);
    xbe[0x210..0x214].copy_from_slice(&0x20000u32.to_le_bytes());
    let specs = Box::leak(
        vec![
            TableSpec {
                name: "weapon_records",
                va: SECTION_VA,
                size: 66 * 0x29c,
                stride: 0x29c,
                count: 66,
                sha256: "",
            },
            TableSpec {
                name: "mode_rows",
                va: SECTION_VA + 0x10000,
                size: 87 * 0x80,
                stride: 0x80,
                count: 87,
                sha256: "",
            },
        ]
        .into_boxed_slice(),
    );
    let last = RAW + 0x10000 + 86 * 0x80;
    xbe[last + 0x20..last + 0x24].copy_from_slice(&0x508390u32.to_le_bytes());
    let owned = OwnedTables::from_xbe_with(&xbe, specs, false).unwrap();
    assert!(owned.weapon_record(65).is_some());
    assert!(owned.weapon_record(66).is_none());
    assert_eq!(owned.mode_row(86).unwrap().record_a.0, 0x508390);
    assert!(owned.mode_row(87).is_none());
}
