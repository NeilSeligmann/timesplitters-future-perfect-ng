// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{GuestMemory, add};

/// Byte-oriented bounded formatter. `args[0]` is the format pointer; conversion
/// arguments start at `args[1]`. Unknown specifiers consume no argument.
/// The returned bytes exclude a terminator and contain at most `capacity - 1` bytes.
///
/// ```
/// use tsfp_kernel_services::{debug, GuestMemory};
/// struct Memory;
/// impl GuestMemory for Memory {
///     fn readable(&self, _: u32, _: usize) -> bool { true }
///     fn read(&self, address: u32, out: &mut [u8]) -> bool {
///         let source = b"_value=%x\0";
///         let Some(bytes) = source.get(address as usize..address as usize + out.len()) else { return false };
///         out.copy_from_slice(bytes);
///         true
///     }
///     fn write(&mut self, _: u32, _: &[u8]) -> bool { false }
/// }
/// assert_eq!(debug::format(&Memory, 1, &[1, 42], 32), b"value=2a");
/// ```
pub fn format(memory: &dyn GuestMemory, address: u32, args: &[u32], capacity: usize) -> Vec<u8> {
    if capacity == 0 {
        return Vec::new();
    }
    let mut output = Vec::new();
    let mut cursor = 1;
    let mut scanned = 0;
    while scanned < 512 {
        let Some(byte) = memory.byte(add(address, scanned)) else {
            output.extend_from_slice(b"<format unreadable>");
            break;
        };
        if byte == 0 {
            break;
        }
        if byte != b'%' {
            output.push(byte);
            scanned += 1;
            continue;
        }
        let spec = if scanned + 1 < 512 {
            memory.byte(add(address, scanned + 1))
        } else {
            None
        };
        match spec {
            Some(b'%') => {
                output.push(b'%');
                scanned += 1;
            }
            Some(spec @ (b'c' | b'd' | b'i' | b'u' | b'x' | b'X' | b'p' | b's')) => {
                if let Some(&value) = args.get(cursor) {
                    cursor += 1;
                    match spec {
                        b'c' => output.push(if (0x20..0x7f).contains(&(value as u8)) {
                            value as u8
                        } else {
                            b'.'
                        }),
                        b'd' | b'i' => {
                            output.extend_from_slice((value as i32).to_string().as_bytes())
                        }
                        b'u' => output.extend_from_slice(value.to_string().as_bytes()),
                        b'x' => output.extend_from_slice(std::format!("{value:x}").as_bytes()),
                        b'X' => output.extend_from_slice(std::format!("{value:X}").as_bytes()),
                        b'p' => output.extend_from_slice(std::format!("0x{value:08x}").as_bytes()),
                        b's' if value == 0 => output.extend_from_slice(b"(null)"),
                        b's' => {
                            for offset in 0..256 {
                                match memory.byte(add(value, offset)) {
                                    None => {
                                        output.extend_from_slice(if offset == 0 {
                                            b"<bad %s ptr>"
                                        } else {
                                            b"<unterminated>"
                                        });
                                        break;
                                    }
                                    Some(0) => break,
                                    Some(byte) => output.push(byte),
                                }
                            }
                        }
                        _ => unreachable!(),
                    }
                } else {
                    output.extend_from_slice(b"<arg?>");
                }
                scanned += 1;
            }
            _ => output.push(b'%'),
        }
        // Truncation does not end the format scan or argument reads, matching C.
        output.truncate(capacity - 1);
        scanned += 1;
    }
    output.truncate(capacity - 1);
    output
}
