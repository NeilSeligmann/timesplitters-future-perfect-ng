//! Animation set entries and per level model tables (T1875, spec in `docs/t-rust-anim.md` section 9).
//!
//! * [`AnimSetEntry`]: one 24 byte entry of the static anim set tables of the executable (the arguments of `0x5B980`
//!   `anim_set_entries_load_raw_files`, 136 call sites, 3713 entries, 2929 distinct ids): name pointer, two policy words
//!   (stored at `0x7DE660 + 8 * slot`), the animation id (the word index of `0x7E03C0`). The clip of an entry is the PAK
//!   entry whose key is `crc32("anim/data/ts3/<name>.raw")` (MEASURED: all 3713 resolve).
//! * [`PropTable`]: the per level model table file `prop/table/prop_c_lv<N>_xbas.raw` (read by `0x242700`, rows copied into
//!   the model table `0x791D4C` by the loop at `0x2429C0`). Layout (MEASURED): `u32` identifier, `u32` pool size S, S bytes of
//!   NUL terminated strings, then 40 byte rows to the end. Row dwords: `[0]` model id (index of the word map `0x78B860`),
//!   `[1]` kind (8 on every character row), `[2]` name offset, `[3]` path offset (both from the pool start; the loader adds
//!   `file + 8` without a zero test, so offset 0 is a valid string), `[4]` -1 or an offset, `[5]` f32 radius (squared by the loader),
//!   `[6]`, `[7]` copied, `[8]` flags (stored at row `+0x28`), `[9]` class index into the table at `0x4FCA60`.
//!   A character row's path `chrs/<name>` selects the `pak/chr.pak` entry with key
//!   `crc32("assets/ts/models/chrs/<name>/xbox/output/<name>.xbr")` ([`chr_entry_key`], MEASURED on 169 of 172 rows).
use tsfp_format_core::{Result, crc32, range, word};

pub const ANIM_CLIP_DIR: &str = "anim/data/ts3/";
pub const PROP_ROW_LEN: usize = 40;

/// One anim set entry (`0x5B980` loop body: name `+0`, policy `+8`/`+0xC`, id word `+0x10`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimSetEntry {
    pub name: String,
    pub policy: [u32; 2],
    pub id: u16,
}

impl AnimSetEntry {
    /// PAK key of the clip file (`0x57F30` hashes the path after the `host0:` prefix).
    pub fn clip_key(&self) -> u32 {
        crc32(format!("{ANIM_CLIP_DIR}{}.raw", self.name).as_bytes())
    }

    /// Parses `ANIMSET <id> <policy0 hex> <policy1 hex> <name>` as written by `tools/t1875_chr_dump.py`.
    pub fn from_line(line: &str) -> Result<Self> {
        let mut parts = line.split_whitespace();
        if parts.next() != Some("ANIMSET") {
            return Err("not an ANIMSET line");
        }
        let mut next = || parts.next().ok_or("short ANIMSET line");
        let id = next()?.parse::<u16>().map_err(|_| "bad id")?;
        let policy0 = u32::from_str_radix(next()?, 16).map_err(|_| "bad policy")?;
        let policy1 = u32::from_str_radix(next()?, 16).map_err(|_| "bad policy")?;
        let name = next()?;
        if parts.next().is_some() {
            return Err("extra ANIMSET field");
        }
        Ok(Self {
            name: name.to_string(),
            policy: [policy0, policy1],
            id,
        })
    }
}

/// One 40 byte model table row, kept verbatim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropRow {
    pub words: [u32; 10],
}

impl PropRow {
    pub fn model_id(&self) -> u32 {
        self.words[0]
    }
    pub fn kind(&self) -> u32 {
        self.words[1]
    }
    pub fn radius(&self) -> f32 {
        f32::from_bits(self.words[5])
    }
    pub fn flags(&self) -> u32 {
        self.words[8]
    }
}

/// A property table retains its string pool and every row word verbatim.
///
/// ```
/// use tsfp_format_anim::propset::PropTable;
/// let table = PropTable { identifier: 7, pool: b"alpha\0".to_vec(), rows: vec![] };
/// let parsed = PropTable::parse(&table.encode())?;
/// assert_eq!(parsed.string(0), Some("alpha"));
/// # Ok::<(), &'static str>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropTable {
    /// Dword 0 (opaque, 0x3F213F02 style identifier on lv1).
    pub identifier: u32,
    pub pool: Vec<u8>,
    pub rows: Vec<PropRow>,
}

impl PropTable {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let identifier = word(bytes, 0)?;
        let pool_len = word(bytes, 4)? as usize;
        let pool = range(bytes, 8, pool_len)?.to_vec();
        let rest = &bytes[8 + pool_len..];
        if rest.len() % PROP_ROW_LEN != 0 {
            return Err("row table is not a multiple of 40 bytes");
        }
        let rows = rest
            .chunks_exact(PROP_ROW_LEN)
            .map(|c| PropRow {
                words: std::array::from_fn(|i| {
                    u32::from_le_bytes(c[4 * i..4 * i + 4].try_into().unwrap())
                }),
            })
            .collect();
        Ok(Self {
            identifier,
            pool,
            rows,
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + self.pool.len() + self.rows.len() * PROP_ROW_LEN);
        out.extend_from_slice(&self.identifier.to_le_bytes());
        out.extend_from_slice(&(self.pool.len() as u32).to_le_bytes());
        out.extend_from_slice(&self.pool);
        for row in &self.rows {
            for w in row.words {
                out.extend_from_slice(&w.to_le_bytes());
            }
        }
        out
    }

    /// NUL terminated string at a pool offset.
    pub fn string(&self, offset: u32) -> Option<&str> {
        let tail = self.pool.get(offset as usize..)?;
        let end = tail.iter().position(|b| *b == 0)?;
        std::str::from_utf8(&tail[..end]).ok()
    }

    pub fn name(&self, row: &PropRow) -> Option<&str> {
        self.string(row.words[2])
    }

    pub fn path(&self, row: &PropRow) -> Option<&str> {
        self.string(row.words[3])
    }
}

/// `pak/chr.pak` key of a model path such as `chrs/deerbeast` (`0x96410` formats
/// `assets/ts/models/%s/xbox/output/%s.xbr` with the path and its last component).
pub fn chr_entry_key(path: &str) -> u32 {
    let base = path.rsplit('/').next().unwrap_or(path);
    crc32(format!("assets/ts/models/{path}/xbox/output/{base}.xbr").as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic() -> Vec<u8> {
        let pool = b"CHR_A\0chrs/alpha\0";
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0xABCDu32.to_le_bytes());
        bytes.extend_from_slice(&(pool.len() as u32).to_le_bytes());
        bytes.extend_from_slice(pool);
        for w in [1430u32, 8, 0, 6, u32::MAX, 0, 0, 0, 517, 0] {
            bytes.extend_from_slice(&w.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn prop_table_round_trips_and_reads_strings() {
        let bytes = synthetic();
        let table = PropTable::parse(&bytes).unwrap();
        assert_eq!(table.encode(), bytes);
        let row = &table.rows[0];
        assert_eq!((row.model_id(), row.kind(), row.flags()), (1430, 8, 517));
        assert_eq!(
            (table.name(row), table.path(row)),
            (Some("CHR_A"), Some("chrs/alpha"))
        );
        assert!(PropTable::parse(&bytes[..bytes.len() - 1]).is_err());
        assert!(PropTable::parse(&bytes[..6]).is_err());
    }

    #[test]
    fn keys_and_set_lines() {
        assert_eq!(
            chr_entry_key("chrs/alpha"),
            crc32(b"assets/ts/models/chrs/alpha/xbox/output/alpha.xbr")
        );
        let entry = AnimSetEntry::from_line("ANIMSET 2009 200283 5800 ashcowerrun").unwrap();
        assert_eq!(
            (entry.id, entry.policy, entry.clip_key()),
            (
                2009,
                [0x200283, 0x5800],
                crc32(b"anim/data/ts3/ashcowerrun.raw")
            )
        );
        assert!(AnimSetEntry::from_line("ANIMSET 1").is_err());
        assert!(AnimSetEntry::from_line("OTHER 1 2 3 x").is_err());
        assert_eq!(
            AnimSetEntry::from_line("ANIMSET 1 2 3 ").unwrap_err(),
            "short ANIMSET line"
        );
        assert_eq!(
            AnimSetEntry::from_line("ANIMSET 1 2 3 x extra").unwrap_err(),
            "extra ANIMSET field"
        );
        assert_eq!(
            AnimSetEntry::from_line("ANIMSET  1 2 3 x\t").unwrap().name,
            "x"
        );
    }

    #[test]
    fn property_grid_preserves_rows_and_pool_boundaries() {
        for row_count in 0..=16 {
            for seed in 0..32u32 {
                let name = format!("model_{seed}");
                let path = format!("chrs/path_{seed}");
                let mut pool = vec![0xa5; seed as usize % 5];
                let name_offset = pool.len() as u32;
                pool.extend_from_slice(name.as_bytes());
                pool.push(0);
                let path_offset = pool.len() as u32;
                pool.extend_from_slice(path.as_bytes());
                pool.push(0);
                let rows = (0..row_count)
                    .map(|index| {
                        let mut words = std::array::from_fn(|word| {
                            seed.wrapping_mul(0x9e37_79b9)
                                .wrapping_add(index as u32 * 31 + word as u32)
                        });
                        words[2] = name_offset;
                        words[3] = path_offset;
                        PropRow { words }
                    })
                    .collect();
                let table = PropTable {
                    identifier: seed,
                    pool,
                    rows,
                };
                let bytes = table.encode();
                let parsed = PropTable::parse(&bytes).unwrap();
                assert_eq!(parsed, table);
                assert_eq!(parsed.encode(), bytes);
                assert_eq!(parsed.string(name_offset), Some(name.as_str()));
                assert_eq!(parsed.string(path_offset), Some(path.as_str()));
                assert_eq!(parsed.string(parsed.pool.len() as u32), None);
                for row in &parsed.rows {
                    assert_eq!(parsed.name(row), Some(name.as_str()));
                    assert_eq!(parsed.path(row), Some(path.as_str()));
                }
            }
        }
    }
}
