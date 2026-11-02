//! Offset-table banks; count is an external loader contract, encoding remains bytes.

use tsfp_format_core::{Result, words};

/// A byte pool indexed by absolute file offsets. The entry count comes from
/// the caller; aliases, offset order, and unused pool bytes are preserved.
///
/// ```
/// use tsfp_format_bs::Bank;
/// let bank = Bank { offsets: vec![9, 8], pool: b"a\0tail".to_vec() };
/// assert_eq!(bank.string(0).unwrap(), b"");
/// assert_eq!(Bank::parse(&bank.encode().unwrap(), 2).unwrap().string(1).unwrap(), b"a");
/// ```
#[derive(Debug, Clone)]
pub struct Bank {
    pub offsets: Vec<u32>,
    pub pool: Vec<u8>,
}

impl Bank {
    /// Parse `count` absolute offsets followed by the retained byte pool.
    pub fn parse(bytes: &[u8], count: usize) -> Result<Self> {
        let offsets = words(bytes, 0, count)?;
        let table_len = count.checked_mul(4).ok_or("count overflow")?;
        let bank = Self {
            offsets,
            pool: bytes[table_len..].to_vec(),
        };
        bank.validate()?;
        Ok(bank)
    }

    fn validate(&self) -> Result<()> {
        for index in 0..self.offsets.len() {
            self.string(index)?;
        }
        Ok(())
    }

    /// Return a zero-terminated string without copying it from the retained pool.
    pub fn string(&self, index: usize) -> Result<&[u8]> {
        let table_len = self.offsets.len().checked_mul(4).ok_or("count overflow")?;
        let absolute_offset = *self.offsets.get(index).ok_or("string index")? as usize;
        let pool_offset = absolute_offset
            .checked_sub(table_len)
            .ok_or("offset in table")?;
        let tail = self.pool.get(pool_offset..).ok_or("offset past pool")?;
        let terminator = tail
            .iter()
            .position(|byte| *byte == 0)
            .ok_or("unterminated string")?;
        Ok(&tail[..terminator])
    }

    /// Write the original offset order and pool after checking every string.
    pub fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let mut bytes = Vec::new();
        for offset in &self.offsets {
            bytes.extend(offset.to_le_bytes());
        }
        bytes.extend(&self.pool);
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::Bank;

    #[test]
    fn aliases() {
        let bank = Bank {
            offsets: vec![14, 12, 14],
            pool: b"a\0b\0unused".to_vec(),
        };
        let raw = bank.encode().unwrap();
        let parsed = Bank::parse(&raw, 3).unwrap();
        assert_eq!(parsed.string(0).unwrap(), b"b");
        assert_eq!(parsed.encode().unwrap(), raw);
        assert!(Bank::parse(&raw, usize::MAX).is_err());
        assert!(
            Bank {
                offsets: vec![4],
                pool: vec![1]
            }
            .encode()
            .is_err()
        );
    }

    #[test]
    fn borrowed_strings_and_error_contract() {
        let bank = Bank {
            offsets: vec![8, 10],
            pool: b"a\0b\0unused".to_vec(),
        };
        let first = bank.string(0).unwrap();
        assert_eq!(first, b"a");
        assert_eq!(first.as_ptr(), bank.pool.as_ptr());
        assert_eq!(bank.string(2), Err("string index"));
        assert_eq!(
            Bank::parse(&bank.encode().unwrap(), 2).unwrap().string(1),
            Ok(b"b".as_slice())
        );
    }

    #[test]
    fn aliases_and_unused_bytes_round_trip() {
        for seed in 0u8..=255 {
            let first = seed % 254 + 1;
            let pool = vec![first, 0, first + 1, 0, 0xff, seed];
            let bank = Bank {
                offsets: vec![14, 12, 14],
                pool: pool.clone(),
            };
            let bytes = bank.encode().unwrap();
            let parsed = Bank::parse(&bytes, 3).unwrap();
            assert_eq!(parsed.string(0).unwrap(), &[pool[2]]);
            assert_eq!(parsed.string(1).unwrap(), &[first]);
            assert_eq!(parsed.string(2).unwrap(), parsed.string(0).unwrap());
            assert_eq!(parsed.pool, pool);
            assert_eq!(parsed.encode().unwrap(), bytes);
        }
    }

    #[test]
    fn bad_offsets_are_rejected() {
        for count in 1usize..=32 {
            let table = count * 4;
            let pool = b"a\0".to_vec();
            for bad in [0, (table - 1) as u32, (table + pool.len()) as u32] {
                let mut offsets = vec![table as u32; count];
                offsets[count - 1] = bad;
                assert!(
                    Bank {
                        offsets,
                        pool: pool.clone()
                    }
                    .encode()
                    .is_err()
                );
            }
        }
    }
}
