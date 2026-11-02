//! Executable Tier 1 differential against the repository C validators, synthetic inputs only.
use std::{fs, process::Command};
use tsfp_audio::scope::{buffer_scope_validate, stream_scope_validate};

const ORACLE: &str = r#"
#include "dsound_stream_scope.h"
#include "dsound_buffer_scope.h"
#include <stdio.h>
#include <string.h>
bool kernel_guest_read_bytes(uint32_t address, void *out, size_t bytes)
{ (void)address; (void)out; (void)bytes; return false; }
int main(int argc, char **argv) {
    if (argc != 2) return 2;
    FILE *f = fopen(argv[1], "rb"); if (!f) return 3;
    unsigned char in[47];
    while (fread(in, 1, sizeof(in), f) == sizeof(in)) {
        dsound_stream_scope s; dsound_buffer_scope b;
        memset(&s, 0xa5, sizeof(s)); memset(&b, 0xa5, sizeof(b));
        dsound_stream_scope old_s = s; dsound_buffer_scope old_b = b;
        int ok = in[0] ? dsound_buffer_scope_validate(in+3,in[1],in+27,in[2],&b)
                       : dsound_stream_scope_validate(in+3,in[1],in+27,in[2],&s);
        if (!ok) {
            if (memcmp(&s,&old_s,sizeof(s)) || memcmp(&b,&old_b,sizeof(b))) return 4;
            puts("none"); continue;
        }
        if (in[0]) printf("%u %u %u %u %u %u %u %u\n", b.descriptor_address,
            b.format_address,b.flags,b.buffer_bytes,b.sample_rate,b.average_bytes_per_second,b.channels,b.block_align);
        else printf("%u %u %u %u %u %u %u %u\n", s.descriptor_address,
            s.format_address,s.flags,s.max_packets,s.sample_rate,s.average_bytes_per_second,s.channels,s.block_align);
        const unsigned char *format = in[0] ? b.format : s.format;
        const unsigned char *desc = in[0] ? b.descriptor : s.descriptor;
        if (memcmp(format,in+27,20) || memcmp(desc,in+3,24)) return 5;
    }
    if (ferror(f)) return 6;
    fclose(f); return 0;
}
"#;

fn word(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn valid(buffer: bool, spatial: bool, address: u32) -> [u8; 47] {
    let mut r = [0; 47];
    r[0] = u8::from(buffer);
    r[1] = 24;
    r[2] = 20;
    let d = &mut r[3..27];
    if buffer {
        word(d, 0, 24);
        word(d, 4, if spatial { 16 } else { 0 });
        word(d, 12, address);
    } else {
        word(d, 0, if spatial { 16 } else { 0 });
        word(d, 4, 3);
        word(d, 8, address);
    }
    let channels: u16 = if buffer || spatial { 1 } else { 2 };
    let rate = if buffer { 44000 } else { 44100 };
    let f = &mut r[27..];
    f[..2].copy_from_slice(&0x69u16.to_le_bytes());
    f[2..4].copy_from_slice(&channels.to_le_bytes());
    word(f, 4, rate);
    word(f, 8, (u32::from(channels) * 36 * rate) >> 6);
    f[12..14].copy_from_slice(&(channels * 36).to_le_bytes());
    f[14] = 4;
    f[16] = 2;
    f[18] = 64;
    r
}
fn expected(r: &[u8; 47]) -> String {
    let d = &r[3..3 + usize::from(r[1])];
    let f = &r[27..27 + usize::from(r[2])];
    let values = if r[0] == 0 {
        stream_scope_validate(d, f).map(|s| {
            [
                s.descriptor_address,
                s.format_address,
                s.flags,
                s.max_packets,
                s.sample_rate,
                s.average_bytes_per_second,
                u32::from(s.channels),
                u32::from(s.block_align),
            ]
        })
    } else {
        buffer_scope_validate(d, f).map(|s| {
            [
                s.descriptor_address,
                s.format_address,
                s.flags,
                s.buffer_bytes,
                s.sample_rate,
                s.average_bytes_per_second,
                u32::from(s.channels),
                u32::from(s.block_align),
            ]
        })
    };
    values
        .map(|v| v.map(|n| n.to_string()).join(" "))
        .unwrap_or_else(|| "none".into())
}

#[test]
fn actual_c_scope_differential() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = std::env::temp_dir().join(format!("tsfp-audio-scope-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("oracle.c"), ORACLE).unwrap();
    // Snapshot functions are linked but not exercised: this stub performs no guest reads.
    fs::write(dir.join("kernel_call.h"),"#include <stdbool.h>\n#include <stdint.h>\n#include <stddef.h>\nbool kernel_guest_read_bytes(uint32_t, void *, size_t);\n").unwrap();
    let result = Command::new(std::env::var("CC").unwrap_or_else(|_| "cc".into()))
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-O2"])
        .arg("-I")
        .arg(&dir)
        .arg("-I")
        .arg(root.join("src/audio"))
        .arg(dir.join("oracle.c"))
        .arg(root.join("crates/tsfp-audio-conformance/oracle/dsound_stream_scope.c"))
        .arg(root.join("crates/tsfp-audio-conformance/oracle/dsound_buffer_scope.c"))
        .arg("-o")
        .arg(dir.join("oracle"))
        .output()
        .expect("C compiler required for differential");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let mut cases = Vec::new();
    let mut seed = 0x2046_51ceu32;
    for buffer in [false, true] {
        for spatial in [false, true] {
            for address in [
                0,
                1,
                0x100300,
                u32::MAX - 20,
                u32::MAX - 19,
                u32::MAX - 18,
                u32::MAX,
            ] {
                cases.push(valid(buffer, spatial, address));
            }
            let base = valid(buffer, spatial, 0x100300);
            for index in 3..47 {
                for bit in 0..8 {
                    let mut r = base;
                    r[index] ^= 1 << bit;
                    cases.push(r);
                }
            }
            for len in 0..24 {
                let mut r = base;
                r[1] = len;
                cases.push(r);
            }
            for len in 0..20 {
                let mut r = base;
                r[2] = len;
                cases.push(r);
            }
            for _ in 0..4096 {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                let mut r = base;
                let index = 3 + (seed as usize % 44);
                r[index] = (seed >> 16) as u8;
                cases.push(r);
            }
        }
    }
    fs::write(
        dir.join("inputs"),
        cases
            .iter()
            .flat_map(|r| r.iter().copied())
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let result = Command::new(dir.join("oracle"))
        .arg(dir.join("inputs"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "C oracle failed: {:?}",
        result.status
    );
    let output = String::from_utf8(result.stdout).unwrap();
    let lines: Vec<_> = output.lines().collect();
    assert_eq!(lines.len(), cases.len());
    let mut accepted = 0;
    for (index, (r, line)) in cases.iter().zip(lines).enumerate() {
        let wanted = expected(r);
        assert_eq!(line, wanted, "case {index}: {r:02x?}");
        accepted += usize::from(line != "none");
    }
    assert!(accepted >= 16, "positive cases must survive");
    eprintln!(
        "C scope differential: {} cases, {accepted} accepted, zero mismatches",
        cases.len()
    );
    fs::remove_dir_all(dir).unwrap();
}
