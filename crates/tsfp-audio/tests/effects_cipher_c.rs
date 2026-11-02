//! Tier 1 differential against the actual C cipher on fabricated byte spans.
use std::{fs, process::Command};
use tsfp_audio::effects_cipher::{Decoder, decode, decode_in_place, setup_key};

#[test]
fn actual_c_effects_cipher() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join(format!("tmp/effects-cipher-oracle-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::copy(
        root.join("crates/tsfp-audio-conformance/oracle/dsound_effects_cipher.c"),
        dir.join("dsound_effects_cipher.c"),
    )
    .unwrap();
    fs::copy(
        root.join("src/audio/dsound_effects_cipher.h"),
        dir.join("dsound_effects_cipher.h"),
    )
    .unwrap();
    fs::write(
        dir.join("oracle.c"),
        r#"
#include <stdio.h>
#include "dsound_effects_cipher.c"
static uint32_t rng = 0x2046c1f3u;
static uint8_t next(void) { rng ^= rng << 13; rng ^= rng >> 17; rng ^= rng << 5; return (uint8_t)rng; }
int main(void) {
    uint8_t key[8], input[257], output[257];
    dsound_effects_cipher_setup(key);
    for (unsigned i=0; i<8; i++) printf("%02x", key[i]);
    putchar('\n');
    for (unsigned case_id=0; case_id<2048; case_id++) {
        size_t length = case_id % 258u;
        if (length == 257u) length = 256u;
        for (size_t j=0; j<length; j++) input[j]=next();
        for (unsigned j=0; j<8; j++) key[j]=next();
        bool ok = dsound_effects_cipher_decode(key,input,output,length);
        printf("%u %u ", case_id, ok);
        for (size_t j=0; j<length; j++) printf("%02x",output[j]);
        putchar('\n');
        dsound_effects_cipher_decode(key,input,input,length);
        printf("I %u ",case_id);
        for (size_t j=0; j<length; j++) printf("%02x",input[j]);
        putchar('\n');
    }
    for (int offset=-7; offset<=7; offset++) {
        uint8_t overlap[80];
        for (size_t j=0; j<sizeof(overlap); j++) overlap[j]=next();
        for (unsigned j=0; j<8; j++) key[j]=next();
        uint8_t *source=overlap+16, *destination=source+offset;
        bool ok = dsound_effects_cipher_decode(key,source,destination,32);
        printf("O %d %u ",offset,ok);
        for (size_t j=0; j<sizeof(overlap); j++) printf("%02x",overlap[j]);
        putchar('\n');
    }
}
"#,
    )
    .unwrap();
    let build = Command::new(std::env::var("CC").unwrap_or_else(|_| "cc".into()))
        .args(["-std=c11", "-O2"])
        .arg(dir.join("oracle.c"))
        .arg("-o")
        .arg(dir.join("oracle"))
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let actual = Command::new(dir.join("oracle")).output().unwrap();
    assert!(actual.status.success());
    let actual = String::from_utf8(actual.stdout).unwrap();
    let mut rng = 0x2046c1f3u32;
    let mut next = || {
        rng ^= rng << 13;
        rng ^= rng >> 17;
        rng ^= rng << 5;
        rng as u8
    };
    let mut expected = String::new();
    for value in setup_key() {
        expected.push_str(&format!("{value:02x}"));
    }
    expected.push('\n');
    for case_id in 0..2048 {
        let length = if case_id % 258 == 257 {
            256
        } else {
            case_id % 258
        };
        let mut input: Vec<u8> = (0..length).map(|_| next()).collect();
        let key = std::array::from_fn(|_| next());
        let mut output = vec![0; length];
        let ok = decode(key, &input, &mut output);
        if std::env::var_os("TSFP_AUDIO_CIPHER_MUTANT").is_some() && case_id == 1 {
            output[0] ^= 1;
        }
        expected.push_str(&format!("{case_id} {} ", u8::from(ok)));
        for value in output {
            expected.push_str(&format!("{value:02x}"));
        }
        expected.push('\n');
        decode_in_place(key, &mut input);
        expected.push_str(&format!("I {case_id} "));
        for value in input {
            expected.push_str(&format!("{value:02x}"));
        }
        expected.push('\n');
    }
    for offset in -7_i32..=7 {
        let mut overlap: Vec<u8> = (0..80).map(|_| next()).collect();
        let key = std::array::from_fn(|_| next());
        let destination_start = (16_i32 + offset) as usize;
        let mut decoder = Decoder::new(key);
        for index in 0..32 {
            let input = overlap[16 + index];
            overlap[destination_start + index] = decoder.decode_byte(input);
        }
        expected.push_str(&format!("O {offset} 1 "));
        for value in overlap {
            expected.push_str(&format!("{value:02x}"));
        }
        expected.push('\n');
    }
    assert_eq!(actual, expected);
    assert!(!decode([0; 8], &[1], &mut []));
    fs::remove_dir_all(dir).unwrap();
}
