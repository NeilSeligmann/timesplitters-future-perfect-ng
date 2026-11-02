use tsfp_import::convert::{BsDoc, path_key, sniff_xbt, xbt_convert, xbt_to_ktx2, xbt_to_png};
use tsfp_import::{inflate, manifest, sha256, source};
use tsfp_import::{names, png};

fn bank() -> Vec<u8> {
    // 3 entries: table 12 bytes, aliasing (entries 0 and 2 share an offset), one unreferenced gap.
    let mut b = Vec::new();
    for o in [12u32, 17, 12] {
        b.extend(o.to_le_bytes());
    }
    b.extend(b"abcd\0");
    b.extend(b"\xe9t\xe9\0");
    b.extend(b"gap!");
    b
}

fn p5ck_members(entries: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut bytes = vec![0u8; 12];
    bytes[..4].copy_from_slice(b"P5CK");
    let mut rows = Vec::new();
    for (key, payload) in entries {
        let offset = bytes.len() as u32;
        bytes.extend_from_slice(payload);
        rows.push((*key, offset, payload.len() as u32, 0u32));
    }
    let toc = bytes.len() as u32;
    bytes[4..8].copy_from_slice(&toc.to_le_bytes());
    bytes[8..12].copy_from_slice(&((rows.len() * 16) as u32).to_le_bytes());
    for (key, offset, size, stored_size) in rows {
        for word in [key, offset, size, stored_size] {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
    }
    bytes
}

fn anrs_sample() -> Vec<u8> {
    let mut bytes = vec![0u8; 256];
    bytes[..4].copy_from_slice(b"ANRS");
    for (offset, value) in [
        (4, 200u32),
        (12, 1),
        (16, 1),
        (20, 1),
        (24, 1),
        (28, 200),
        (32, 208),
        (36, 212),
    ] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn mkr_sample() -> Vec<u8> {
    use tsfp_format_script::mkr::{Chunk, Mkr, TAG_RULE, TAG_SETUP, VERSION};
    Mkr {
        version: VERSION,
        chunks: vec![
            Chunk {
                tag: TAG_SETUP,
                payload: vec![0x21; 0xc0],
            },
            Chunk {
                tag: TAG_RULE,
                payload: vec![0x34; 0x220],
            },
            Chunk {
                tag: 0x00c7_766b,
                payload: vec![0xa5, 0x5a],
            },
        ],
        tail: vec![0x7f; 20],
    }
    .encode()
    .unwrap()
}

#[test]
fn sha256_vectors() {
    assert_eq!(
        sha256::hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        sha256::hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn bs_roundtrip_keeps_alias_and_gaps() {
    let b = bank();
    let d = BsDoc::parse(&b).unwrap();
    assert_eq!(d.gaps.len(), 1);
    assert_eq!(d.strings[0], d.strings[2]);
    assert_eq!(d.to_bytes(), b);
    let j = d.to_json();
    assert!(j.contains("\"count\":3") && j.contains('\u{e9}'));
}

#[test]
fn bs_rejects_garbage() {
    assert!(BsDoc::parse(&[1, 0, 0, 0]).is_err());
    assert!(BsDoc::parse(b"").is_err());
}

#[test]
fn gunzip_stored_and_fixed() {
    // gzip of "hello" (fixed Huffman, produced by gzip -n).
    let g = [
        0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 3, 0xcb, 0x48, 0xcd, 0xc9, 0xc9, 7, 0, 0x86, 0xa6, 0x10,
        0x36, 5, 0, 0, 0,
    ];
    assert_eq!(inflate::gunzip(&g, 100).unwrap(), b"hello");
    assert!(inflate::gunzip(&g, 2).is_err());
    let mut bad = g;
    bad[17] ^= 1;
    assert!(inflate::gunzip(&bad, 100).is_err());
}

fn xbt(w: u32, h: u32, fmt: u32, pixels: usize) -> Vec<u8> {
    let mut b = Vec::new();
    for x in [w, h, w, h, 0, fmt, 0, 0, 0] {
        b.extend(x.to_le_bytes());
    }
    b.extend([0u8; 92]);
    b.extend(vec![7u8; pixels]);
    b
}

#[test]
fn xbt_dxt1_to_ktx2_keeps_blocks_and_mips() {
    let mut source = xbt(8, 8, 0, 4 * 8 + 8 + 8);
    for (index, byte) in source[128..].iter_mut().enumerate() {
        *byte = index as u8;
    }
    let (ktx, note) = xbt_to_ktx2(&source).unwrap();
    assert_eq!(&ktx[..12], b"\xabKTX 20\xbb\r\n\x1a\n");
    assert_eq!(u32::from_le_bytes(ktx[12..16].try_into().unwrap()), 133); // BC1 RGBA
    assert_eq!(u32::from_le_bytes(ktx[16..20].try_into().unwrap()), 1);
    assert_eq!(u32::from_le_bytes(ktx[20..24].try_into().unwrap()), 8);
    assert_eq!(u32::from_le_bytes(ktx[24..28].try_into().unwrap()), 8);
    assert_eq!(u32::from_le_bytes(ktx[40..44].try_into().unwrap()), 3);
    assert_eq!(u32::from_le_bytes(ktx[44..48].try_into().unwrap()), 0); // no supercompression
    let dfd_offset = u32::from_le_bytes(ktx[48..52].try_into().unwrap()) as usize;
    assert_eq!(dfd_offset, 80 + 3 * 24);
    assert_eq!(u32::from_le_bytes(ktx[52..56].try_into().unwrap()), 44);
    assert_eq!(
        u32::from_le_bytes(ktx[dfd_offset..dfd_offset + 4].try_into().unwrap()),
        44
    );
    assert_eq!(ktx[dfd_offset + 12], 128); // KHR_DF_MODEL_BC1A
    assert_eq!(ktx[dfd_offset + 31], 1); // DXT1 punch-through alpha semantics
    assert!(note.contains("mips 3 preserved"));

    let mut source_offset = 0;
    let mut previous_level_end = 0;
    for (index, expected_len) in [32usize, 8, 8].into_iter().enumerate() {
        let entry = 80 + index * 24;
        let level_offset = u64::from_le_bytes(ktx[entry..entry + 8].try_into().unwrap()) as usize;
        let level_len = u64::from_le_bytes(ktx[entry + 8..entry + 16].try_into().unwrap()) as usize;
        let unpacked_len =
            u64::from_le_bytes(ktx[entry + 16..entry + 24].try_into().unwrap()) as usize;
        assert_eq!(level_offset % 8, 0);
        assert!(level_offset >= previous_level_end);
        assert_eq!((level_len, unpacked_len), (expected_len, expected_len));
        assert_eq!(
            &ktx[level_offset..level_offset + level_len],
            &source[128 + source_offset..128 + source_offset + expected_len]
        );
        source_offset += expected_len;
        previous_level_end = level_offset + level_len;
    }
    assert_eq!(source_offset, source.len() - 128);
    assert!(xbt_to_ktx2(&xbt(8, 8, 0, 33)).is_err());
    assert!(xbt_to_ktx2(&xbt(8, 8, 2, 256)).is_err());
    assert!(xbt_to_png(&xbt(8, 8, 2, 255)).is_err());
}

#[test]
fn xbt_dxt3_and_dxt5_map_to_bc2_and_bc3() {
    for (xbt_format, vk_format, model) in [(1u32, 135u32, 129u8), (4, 137, 130)] {
        let source = xbt(4, 4, xbt_format, 16);
        let (ktx, _) = xbt_to_ktx2(&source).unwrap();
        let dfd_offset = u32::from_le_bytes(ktx[48..52].try_into().unwrap()) as usize;
        assert_eq!(
            u32::from_le_bytes(ktx[12..16].try_into().unwrap()),
            vk_format
        );
        assert_eq!(u32::from_le_bytes(ktx[52..56].try_into().unwrap()), 60);
        assert_eq!(ktx[dfd_offset + 12], model);
        let level_offset = u64::from_le_bytes(ktx[80..88].try_into().unwrap()) as usize;
        assert_eq!(level_offset % 16, 0);
        assert_eq!(&ktx[level_offset..level_offset + 16], &source[128..144]);
    }
}

#[test]
fn pak_key_is_crc32_of_path() {
    assert_eq!(
        path_key("textures/cc95c8.xbt"),
        tsfp_format_core::crc32(b"textures/cc95c8.xbt")
    );
}

#[test]
fn manifest_rows_roundtrip_and_paths_are_safe() {
    let rows = vec![manifest::StageAFile {
        path: "a \"q\"/b.bin".into(),
        size: 3,
        sha256: "ab".into(),
    }];
    let t = manifest::stage_a_json("xbox-usa", "xbox", "xdvdfs", &rows);
    let r = manifest::parse_rows(&t).unwrap();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0]["path"], "a \"q\"/b.bin");
    assert_eq!(r[0]["size"], "3");
    assert!(
        source::safe("../x").is_err() && source::safe("/x").is_err() && source::safe("a/b").is_ok()
    );
}

#[test]
fn directory_source_extract_convert_verify() {
    let tmp = std::env::temp_dir().join(format!("tsfp-import-test-{}", std::process::id()));
    let (src, out) = (tmp.join("src"), tmp.join("out"));
    std::fs::create_dir_all(src.join("data")).unwrap();
    std::fs::write(src.join("data/x.bs"), bank()).unwrap();
    let s = source::open(&src, "gc-test", source::Platform::GameCube).unwrap();
    assert_eq!(s.list().unwrap().len(), 1);
    tsfp_import::convert::stage_a(s.as_ref(), &out, "").unwrap();
    let o = tsfp_import::convert::stage_b(s.as_ref(), &out, "", &[]).unwrap();
    assert_eq!(o.len(), 1);
    assert!(o[0].lossless);
    assert_eq!(tsfp_import::convert::verify(&out, "gc-test").unwrap(), 2);
    let version = out.join("assets/original/gc-test");
    assert!(version.join("text/data/x.json").is_file());
    assert!(version.join("manifest.sha256").is_file());
    tsfp_import::convert::write_override_contract(&out).unwrap();
    assert!(out.join("assets/overrides/.contract.json").is_file());
    std::fs::write(version.join(".source/data/x.bs"), b"tampered").unwrap();
    assert!(tsfp_import::convert::verify(&out, "gc-test").is_err());
    std::fs::remove_dir_all(&tmp).unwrap();
}

#[test]
fn xmv_files_and_named_pak_members_are_copied_verbatim() {
    let tmp = std::env::temp_dir().join(format!("tsfp-import-xmv-{}", std::process::id()));
    let (src, out) = (tmp.join("src"), tmp.join("out"));
    std::fs::create_dir_all(src.join("pak")).unwrap();
    let loose = b"XMV\0 loose bytes including opaque packets";
    let packed = b"XMV\0 named PAK member bytes including opaque packets";
    std::fs::write(src.join("loose.xmv"), loose).unwrap();
    let member_name = "movies/intro.xmv";
    let member_key = tsfp_format_core::crc32(member_name.as_bytes());
    std::fs::write(
        src.join("pak/cutscene.pak"),
        pak(&[(member_key, packed.to_vec())]),
    )
    .unwrap();

    let s = source::open(&src, "gc-xmv", source::Platform::GameCube).unwrap();
    tsfp_import::convert::stage_a(s.as_ref(), &out, "").unwrap();
    let outputs =
        tsfp_import::convert::stage_b(s.as_ref(), &out, "", &[member_name.into()]).unwrap();
    assert_eq!(outputs.len(), 2);
    assert!(
        outputs
            .iter()
            .all(|row| row.converter == "xmv-raw" && row.lossless)
    );
    let version = out.join("assets/original/gc-xmv");
    assert_eq!(
        std::fs::read(version.join("movies/loose.xmv")).unwrap(),
        loose
    );
    assert_eq!(
        std::fs::read(version.join("movies/movies/intro.xmv")).unwrap(),
        packed
    );
    assert_eq!(tsfp_import::convert::verify(&out, "gc-xmv").unwrap(), 4);
    std::fs::remove_dir_all(&tmp).unwrap();
}

#[test]
fn structured_members_convert_only_after_exact_roundtrip() {
    let tmp = std::env::temp_dir().join(format!("tsfp-import-structured-{}", std::process::id()));
    let (src, out) = (tmp.join("src"), tmp.join("out"));
    std::fs::create_dir_all(src.join("maps")).unwrap();
    std::fs::create_dir_all(src.join("pak")).unwrap();
    std::fs::write(src.join("maps/anicemap.mkr"), mkr_sample()).unwrap();
    std::fs::write(src.join("maps/bad.mkr"), b"not an mkr").unwrap();
    let csinfo_name = "assets/ts/cutscenes/sample/xbox/output/csinfo.txt";
    let bad_csinfo_name = "assets/ts/cutscenes/bad/xbox/output/csinfo.txt";
    std::fs::write(
        src.join("pak/cutscene.pak"),
        p5ck_members(&[
            (
                path_key(csinfo_name),
                b"FRAMES 30\nSTREAM scene track\n".to_vec(),
            ),
            (path_key(bad_csinfo_name), b"FRAMES 0\n".to_vec()),
            (0x8765_4321, b"FRAMES 0\n".to_vec()),
            (0x1234_5678, anrs_sample()),
        ]),
    )
    .unwrap();

    let source = source::open(&src, "structured", source::Platform::GameCube).unwrap();
    tsfp_import::convert::stage_a(source.as_ref(), &out, "").unwrap();
    let (outputs, report) = tsfp_import::convert::stage_b_report(
        source.as_ref(),
        &out,
        "",
        &[csinfo_name.into(), bad_csinfo_name.into()],
    )
    .unwrap();

    for converter in ["mkr-json", "csinfo-json", "igcs-json"] {
        assert!(outputs.iter().any(|output| output.converter == converter));
    }
    assert!(outputs.iter().any(|output| {
        output.converter == "csinfo-json" && output.logical_id.ends_with("/csinfo.json")
    }));
    let raw_notes = outputs
        .iter()
        .filter(|output| output.converter == "raw-member")
        .map(|output| output.note.as_str())
        .collect::<Vec<_>>();
    assert_eq!(raw_notes.len(), 3);
    assert!(
        raw_notes
            .iter()
            .any(|note| note.contains("MKR parser rejected"))
    );
    assert!(
        raw_notes
            .iter()
            .any(|note| note.contains("csinfo parser rejected"))
    );
    assert_eq!(report.named_members, 2);
    assert!(outputs.iter().all(|output| output.lossless));
    assert_eq!(
        tsfp_import::convert::verify(&out, "structured").unwrap(),
        source.list().unwrap().len() + outputs.len()
    );
    std::fs::remove_dir_all(&tmp).unwrap();
}

#[test]
fn lossy_texture_keeps_raw_and_manifest_hash_is_checked() {
    let tmp = std::env::temp_dir().join(format!("tsfp-import-lossy-{}", std::process::id()));
    let (src, out) = (tmp.join("src"), tmp.join("out"));
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("mip.xbt"), xbt(8, 8, 2, 320)).unwrap();
    let s = source::open(&src, "lossy", source::Platform::GameCube).unwrap();
    tsfp_import::convert::stage_a(s.as_ref(), &out, "").unwrap();
    let outputs = tsfp_import::convert::stage_b(s.as_ref(), &out, "", &[]).unwrap();
    assert_eq!(outputs.len(), 2);
    assert!(outputs.iter().any(|o| !o.lossless));
    assert!(outputs.iter().any(|o| o.converter == "raw-member"));
    assert_eq!(tsfp_import::convert::verify(&out, "lossy").unwrap(), 3);

    std::fs::write(
        out.join("assets/original/lossy/manifest.sha256"),
        "0000000000000000000000000000000000000000000000000000000000000000  manifest.json\n",
    )
    .unwrap();
    assert!(tsfp_import::convert::verify(&out, "lossy").is_err());
    std::fs::remove_dir_all(&tmp).unwrap();
}

#[test]
fn rvz_is_refused_with_guidance() {
    let p = std::env::temp_dir().join(format!("tsfp-import-rvz-{}.rvz", std::process::id()));
    std::fs::write(&p, b"RVZ\x01padding").unwrap();
    let e = source::open(&p, "gc", source::Platform::GameCube)
        .err()
        .unwrap();
    assert!(e.contains("RVZ"));
    std::fs::remove_file(&p).unwrap();
}

/// Real-data check, skipped unless TSFP_XBOX_ISO names the owner's image.
#[test]
fn real_xbox_image_text_and_texture() {
    let Ok(iso) = std::env::var("TSFP_XBOX_ISO") else {
        eprintln!("skip: TSFP_XBOX_ISO unset");
        return;
    };
    let s = source::open(
        std::path::Path::new(&iso),
        "xbox-usa",
        source::Platform::Xbox,
    )
    .unwrap();
    let files = s.list().unwrap();
    assert!(files.len() > 100);
    let pak = files
        .iter()
        .find(|f| f.path == "pak/arcade/l_103.pak")
        .unwrap();
    let data = s.read(pak).unwrap();
    let p = tsfp_format_pak::Pak::parse(&data).unwrap();
    let i = p
        .entries
        .iter()
        .position(|e| e.key == path_key("data/str_ts_pxe.bs"))
        .unwrap();
    let e = &p.entries[i];
    let body = if e.stored_size != 0 {
        inflate::gunzip(p.stored(i).unwrap(), e.decoded_size as usize).unwrap()
    } else {
        p.stored(i).unwrap().to_vec()
    };
    assert_eq!(body.len(), 119230);
    let d = BsDoc::parse(&body).unwrap();
    assert_eq!(d.offsets.len(), 3463);
    assert_eq!(d.to_bytes(), body);
}

#[test]
fn morton_is_a_bijection_and_matches_known_order() {
    for (w, h) in [(8, 8), (16, 4), (2, 32), (1, 1), (64, 64)] {
        let mut seen = vec![false; w * h];
        for y in 0..h {
            for x in 0..w {
                let m = png::morton(x, y, w, h);
                assert!(m < w * h && !seen[m]);
                seen[m] = true;
            }
        }
    }
    // x takes the even bits, y the odd bits
    assert_eq!(
        [(1, 0), (0, 1), (1, 1), (2, 0), (0, 2)].map(|(x, y)| png::morton(x, y, 4, 4)),
        [1, 2, 3, 4, 8]
    );
    // 16x4: after the two y bits are used the rest of x continues linearly
    assert_eq!(png::morton(4, 0, 16, 4), 16);
    assert_eq!(png::morton(0, 3, 16, 4), 10);
}

#[test]
fn unswizzle_inverts_a_swizzle() {
    let (w, h) = (16, 8);
    let lin: Vec<u8> = (0..w * h * 2).map(|i| (i * 7 + 1) as u8).collect();
    let mut sw = vec![0u8; lin.len()];
    for y in 0..h {
        for x in 0..w {
            let m = png::morton(x, y, w, h) * 2;
            sw[m..m + 2].copy_from_slice(&lin[(y * w + x) * 2..(y * w + x) * 2 + 2]);
        }
    }
    assert_eq!(png::unswizzle(&sw, w, h, 2), lin);
}

#[test]
fn png_container_is_well_formed() {
    let p = png::encode(2, 2, png::Color::Rgba, &[1; 16]);
    assert_eq!(&p[..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(&p[12..16], b"IHDR");
    assert_eq!(u32::from_be_bytes(p[16..20].try_into().unwrap()), 2);
    assert_eq!(&p[p.len() - 8..p.len() - 4], b"IEND");
    // the IHDR checksum covers kind and data
    assert_eq!(
        u32::from_be_bytes(p[29..33].try_into().unwrap()),
        tsfp_format_core::crc32(&p[12..29])
    );
}

#[test]
fn png_stored_blocks_preserve_scanlines_across_boundaries() {
    for color in [
        png::Color::Gray,
        png::Color::GrayAlpha,
        png::Color::Rgb,
        png::Color::Rgba,
    ] {
        for width in [1, 255, 16384, 65534, 65535, 65536] {
            let height = 2;
            let pixels: Vec<u8> = (0..width * height * color.channels())
                .map(|i| (i.wrapping_mul(37) ^ (i >> 7)) as u8)
                .collect();
            let bytes = png::encode(width, height, color, &pixels);
            let mut pos = 8;
            let mut idat = None;
            for kind in [b"IHDR", b"IDAT", b"IEND"] {
                let len = u32::from_be_bytes(bytes[pos..pos + 4].try_into().unwrap()) as usize;
                assert_eq!(&bytes[pos + 4..pos + 8], kind);
                let body = &bytes[pos + 4..pos + 8 + len];
                assert_eq!(
                    u32::from_be_bytes(bytes[pos + 8 + len..pos + 12 + len].try_into().unwrap()),
                    tsfp_format_core::crc32(body)
                );
                if kind == b"IHDR" {
                    assert_eq!(
                        u32::from_be_bytes(bytes[pos + 8..pos + 12].try_into().unwrap()),
                        width as u32
                    );
                    assert_eq!(bytes[pos + 17], color as u8);
                } else if kind == b"IDAT" {
                    idat = Some(&bytes[pos + 8..pos + 8 + len]);
                }
                pos += len + 12;
            }
            assert_eq!(pos, bytes.len());
            let z = idat.unwrap();
            assert_eq!(&z[..2], &[0x78, 0x01]);
            let mut cursor = 2;
            let mut raw = Vec::new();
            loop {
                let final_block = z[cursor];
                assert!(final_block <= 1);
                let len = u16::from_le_bytes(z[cursor + 1..cursor + 3].try_into().unwrap());
                let complement = u16::from_le_bytes(z[cursor + 3..cursor + 5].try_into().unwrap());
                assert_eq!(len ^ complement, u16::MAX);
                cursor += 5;
                raw.extend_from_slice(&z[cursor..cursor + len as usize]);
                cursor += len as usize;
                if final_block == 1 {
                    break;
                }
            }
            let mut expected = Vec::new();
            for row in pixels.chunks(width * color.channels()) {
                expected.push(0);
                expected.extend_from_slice(row);
            }
            assert_eq!(raw, expected);
            let mut a = 1u32;
            let mut b = 0u32;
            for byte in &raw {
                a = (a + u32::from(*byte)) % 65521;
                b = (b + a) % 65521;
            }
            assert_eq!(&z[cursor..], &((b << 16) | a).to_be_bytes());
        }
    }
}

#[test]
fn uncompressed_xbt_formats_convert_and_sniff() {
    for (fmt, bpp) in [(2u32, 4usize), (3, 2), (5, 1), (6, 1), (7, 2)] {
        let b = xbt(8, 4, fmt, 8 * 4 * bpp);
        let (ext, _, out, note) = xbt_convert(&b).unwrap();
        assert_eq!(ext, "png");
        assert_eq!(&out[1..4], b"PNG");
        assert!(note.contains("unswizzled"));
        assert!(sniff_xbt(&b));
    }
    // A8R8G8B8 channel order: memory B,G,R,A becomes R,G,B,A (1x1 so swizzle is the identity)
    let one = xbt(1, 1, 2, 4);
    let mut one = one;
    one[128..132].copy_from_slice(&[10, 20, 30, 40]);
    let (png_bytes, _) = xbt_to_png(&one).unwrap();
    assert!(png_bytes.windows(4).any(|w| w == [30, 20, 10, 40]));
    assert!(!sniff_xbt(&xbt(8, 8, 2, 7)));
    assert!(!sniff_xbt(&[0u8; 200]));
}

#[test]
fn uncompressed_xbt_png_round_trip_matches_each_decoded_texel_format() {
    for (format, bpp, encoded, color, expected) in [
        (
            2u32,
            4usize,
            vec![10, 20, 30, 40],
            png::Color::Rgba,
            vec![30, 20, 10, 40],
        ),
        (3, 2, vec![0x00, 0xf8], png::Color::Rgb, vec![255, 0, 0]),
        (5, 1, vec![128], png::Color::GrayAlpha, vec![255, 128]),
        (6, 1, vec![127], png::Color::Gray, vec![127]),
        (7, 2, vec![50, 200], png::Color::GrayAlpha, vec![50, 200]),
    ] {
        let mut source = xbt(1, 1, format, bpp);
        source[128..].copy_from_slice(&encoded);
        let (converted, _) = xbt_to_png(&source).unwrap();
        png::verify_encoded_pixels(&converted, 1, 1, color, &expected).unwrap();
        let mut wrong_pixels = expected;
        wrong_pixels[0] ^= 1;
        assert!(png::verify_encoded_pixels(&converted, 1, 1, color, &wrong_pixels).is_err());
    }
}

#[test]
fn name_recovery_matches_crc_keys_and_filters_noise() {
    let xbe = b"\0junk\0Music\\Cs03_Test.XBM\0anim/data/x.raw\0d3^6b\0".to_vec();
    let t = names::table(&[], &["data/a.bs".to_string()], &[xbe]);
    assert_eq!(t[&path_key("music/cs03_test.xbm")], "music/cs03_test.xbm");
    assert_eq!(t[&path_key("anim/data/x.raw")], "anim/data/x.raw");
    assert_eq!(t[&path_key("data/a.bs")], "data/a.bs");
    assert!(!t.values().any(|v| v.contains('^')));
    let keys = [path_key("anim/data/x.raw")].into_iter().collect();
    assert_eq!(names::recover(&keys, ["Anim/Data/X.raw", "zzz"]).len(), 1);
}

#[test]
fn stage_b_is_incremental() {
    let tmp = std::env::temp_dir().join(format!("tsfp-import-inc-{}", std::process::id()));
    let (src, out) = (tmp.join("src"), tmp.join("out"));
    std::fs::create_dir_all(src.join("data")).unwrap();
    std::fs::write(src.join("data/x.bs"), bank()).unwrap();
    std::fs::write(src.join("a.xbt"), xbt(8, 4, 6, 32)).unwrap();
    let s = source::open(&src, "inc", source::Platform::GameCube).unwrap();
    let (o1, r1) = tsfp_import::convert::stage_b_report(s.as_ref(), &out, "", &[]).unwrap();
    assert_eq!((o1.len(), r1.converted_files, r1.reused_files), (2, 2, 0));
    let (o2, r2) = tsfp_import::convert::stage_b_report(s.as_ref(), &out, "", &[]).unwrap();
    assert_eq!((o2.len(), r2.converted_files, r2.reused_files), (2, 0, 2));
    // change one source: only that file is redone
    let mut changed = xbt(8, 4, 6, 32);
    changed[130] = 9;
    std::fs::write(src.join("a.xbt"), changed).unwrap();
    bump_mtime(&src.join("a.xbt"));
    let (_, r3) = tsfp_import::convert::stage_b_report(s.as_ref(), &out, "", &[]).unwrap();
    assert_eq!((r3.converted_files, r3.reused_files), (1, 1));
    // a different name list changes the fingerprint and redoes everything
    let (_, r4) =
        tsfp_import::convert::stage_b_report(s.as_ref(), &out, "", &["x/y.raw".into()]).unwrap();
    assert_eq!(r4.reused_files, 0);
    // a stale file from an older converter is pruned on a full run
    let stale = out.join("assets/original/inc/old/leftover.bin");
    std::fs::create_dir_all(stale.parent().unwrap()).unwrap();
    std::fs::write(&stale, b"x").unwrap();
    tsfp_import::convert::stage_b_report(s.as_ref(), &out, "", &["x/y.raw".into()]).unwrap();
    assert!(!stale.exists() && !out.join("assets/original/inc/old").exists());
    // a deleted output forces a redo of its file
    std::fs::remove_file(out.join("assets/original/inc/text/data/x.json")).unwrap();
    let (_, r5) =
        tsfp_import::convert::stage_b_report(s.as_ref(), &out, "", &["x/y.raw".into()]).unwrap();
    assert_eq!((r5.converted_files, r5.reused_files), (1, 1));
    std::fs::remove_dir_all(&tmp).unwrap();
}

/// Real-data check: a 512x256 sky texture must unswizzle into a smooth image.
#[test]
fn real_xbox_uncompressed_texture_is_smooth_after_unswizzle() {
    let Ok(iso) = std::env::var("TSFP_XBOX_ISO") else {
        eprintln!("skip: TSFP_XBOX_ISO unset");
        return;
    };
    let s = source::open(
        std::path::Path::new(&iso),
        "xbox-usa",
        source::Platform::Xbox,
    )
    .unwrap();
    let files = s.list().unwrap();
    let pak = files
        .iter()
        .find(|f| f.path == "pak/arcademem/l_25_arm.pak")
        .unwrap();
    let data = s.read(pak).unwrap();
    let p = tsfp_format_pak::Pak::parse(&data).unwrap();
    let i = p.entries.iter().position(|e| e.key == 0x724460c9).unwrap();
    let e = &p.entries[i];
    let body = if e.stored_size != 0 {
        inflate::gunzip(p.stored(i).unwrap(), e.decoded_size as usize).unwrap()
    } else {
        p.stored(i).unwrap().to_vec()
    };
    assert!(sniff_xbt(&body));
    let t = tsfp_format_xbt::Texture::parse(&body).unwrap();
    assert_eq!((t.words[0], t.words[1], t.words[5]), (512, 256, 2));
    let lin = png::unswizzle(&t.pixels, 512, 256, 4);
    let diff = |img: &[u8]| {
        (0..img.len() - 4)
            .step_by(4)
            .map(|i| (img[i] as i32 - img[i + 4] as i32).unsigned_abs() as u64)
            .sum::<u64>()
    };
    // reading the swizzled bytes as linear is rougher than the unswizzled image (measured 168147 vs 129733 on this dark sky; the image was also checked visually)
    assert!(
        diff(&lin) * 10 < diff(&t.pixels) * 9,
        "unswizzled {} swizzled {}",
        diff(&lin),
        diff(&t.pixels)
    );
}

/// Move a file's modification time forward so stamp based caches see a change on any filesystem.
fn bump_mtime(p: &std::path::Path) {
    let f = std::fs::File::options().write(true).open(p).unwrap();
    f.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(5))
        .unwrap();
}

fn be(v: u32) -> [u8; 4] {
    v.to_be_bytes()
}

/// Smallest GCM with the structures the reader needs: header magic, FST pointer, DOL, apploader.
fn gcm_image() -> Vec<u8> {
    let mut img = vec![0u8; 0x8000];
    img[..6].copy_from_slice(b"GTSE4Z");
    img[0x1C..0x20].copy_from_slice(&0xC2339F3Du32.to_be_bytes());
    img[0x2454..0x2458].copy_from_slice(&be(0x10)); // apploader size
    img[0x2458..0x245C].copy_from_slice(&be(0x20)); // trailer size
    // DOL at 0x3000: one text section at file offset 0x100, 0x40 bytes
    img[0x420..0x424].copy_from_slice(&be(0x3000));
    img[0x3000..0x3004].copy_from_slice(&be(0x100));
    img[0x3090..0x3094].copy_from_slice(&be(0x40));
    // FST at 0x4000: root, dir "d" (entries 1..3), file "d/x.bs", file "y.bin"
    let names = b"d\0x.bs\0y.bin\0";
    let entry = |dir: u8, name: u32, a: u32, b: u32| {
        let mut e = vec![dir];
        e.extend(&name.to_be_bytes()[1..]);
        e.extend(be(a));
        e.extend(be(b));
        e
    };
    let mut fst = Vec::new();
    fst.extend(entry(1, 0, 0, 4));
    fst.extend(entry(1, 0, 0, 3));
    fst.extend(entry(0, 2, 0x5000, 5));
    fst.extend(entry(0, 7, 0x5010, 3));
    fst.extend(names);
    img[0x424..0x428].copy_from_slice(&be(0x4000));
    img[0x428..0x42C].copy_from_slice(&be(fst.len() as u32));
    img[0x4000..0x4000 + fst.len()].copy_from_slice(&fst);
    img[0x5000..0x5005].copy_from_slice(b"hello");
    img[0x5010..0x5013].copy_from_slice(b"yyy");
    img
}

#[test]
fn gcm_fst_is_read_with_nested_directories_and_sys_files() {
    let p = std::env::temp_dir().join(format!("tsfp-import-gcm-{}.iso", std::process::id()));
    std::fs::write(&p, gcm_image()).unwrap();
    let s = source::open(&p, "gc-t", source::Platform::GameCube).unwrap();
    assert_eq!(s.descriptor().layout, "gcm");
    let files = s.list().unwrap();
    let get = |n: &str| {
        files
            .iter()
            .find(|f| f.path == n)
            .unwrap_or_else(|| panic!("{n}"))
    };
    assert_eq!(s.read(get("files/d/x.bs")).unwrap(), b"hello");
    assert_eq!(s.read(get("files/y.bin")).unwrap(), b"yyy");
    assert_eq!(get("sys/main.dol").size, 0x140);
    assert_eq!(get("sys/apploader.img").size, 0x50);
    assert_eq!(get("sys/boot.bin").size, 0x440);
    assert!(!s.stamp(get("files/y.bin")).is_empty());
    // a corrupt FST entry count is refused, not read out of range
    let mut bad = gcm_image();
    bad[0x4008..0x400C].copy_from_slice(&be(100000));
    std::fs::write(&p, bad).unwrap();
    let s = source::open(&p, "gc-t", source::Platform::GameCube).unwrap();
    assert!(s.list().is_err());
    std::fs::remove_file(&p).unwrap();
}

fn xblock(pred: i16, index: i16, byte: u8) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend(pred.to_le_bytes());
    b.extend(index.to_le_bytes());
    b.extend([byte; 32]);
    b
}

fn decode_flac(encoded: &[u8]) -> ((u32, u32, u32), Vec<i32>) {
    assert!(
        encoded.len() >= 42,
        "truncated FLAC stream: {} bytes",
        encoded.len()
    );
    assert_eq!(&encoded[..4], b"fLaC");
    assert_eq!(
        encoded[4] & 0x7f,
        0,
        "first metadata block must be STREAMINFO"
    );
    let metadata_len =
        (u32::from(encoded[5]) << 16) | (u32::from(encoded[6]) << 8) | u32::from(encoded[7]);
    assert_eq!(metadata_len, 34);
    let packed = u64::from_be_bytes(encoded[18..26].try_into().unwrap());
    let rate = (packed >> 44) as u32;
    let channels = ((packed >> 41) & 7) as u32 + 1;
    let bits = ((packed >> 36) & 31) as u32 + 1;
    let total_frames = (packed & 0x0f_ffff_ffff) as usize;
    assert_eq!(
        bits, 16,
        "the test decoder handles the importer's 16-bit PCM profile"
    );
    let mut samples = vec![0i32; total_frames * channels as usize];
    let (mut cursor, mut first_frame) = (42, 0usize);
    while first_frame < total_frames {
        let start = cursor;
        assert_eq!(encoded[cursor], 0xff);
        assert_eq!(encoded[cursor + 1], 0xf9);
        let block_code = encoded[cursor + 2] >> 4;
        assert_eq!(block_code, 6, "expected an 8-bit block-size field");
        cursor += 4;
        let utf_lead = encoded[cursor];
        let utf_width = if utf_lead & 0x80 == 0 {
            1
        } else {
            utf_lead.leading_ones() as usize
        };
        cursor += utf_width;
        let block_frames = usize::from(encoded[cursor]) + 1;
        cursor += 1; // block-size byte
        assert_eq!(test_flac_crc8(&encoded[start..cursor]), encoded[cursor]);
        cursor += 1; // header CRC-8
        for channel in 0..channels as usize {
            assert_eq!(encoded[cursor], 0x02, "expected a verbatim subframe");
            cursor += 1;
            for frame in 0..block_frames {
                let sample = i16::from_be_bytes(encoded[cursor..cursor + 2].try_into().unwrap());
                cursor += 2;
                samples[(first_frame + frame) * channels as usize + channel] = i32::from(sample);
            }
        }
        let expected_crc = u16::from_be_bytes(encoded[cursor..cursor + 2].try_into().unwrap());
        assert_eq!(test_flac_crc16(&encoded[start..cursor]), expected_crc);
        cursor += 2; // frame CRC-16
        assert!(cursor > start);
        first_frame += block_frames;
    }
    assert_eq!(cursor, encoded.len(), "unexpected trailing FLAC data");
    ((rate, channels, bits), samples)
}

fn test_flac_crc8(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |mut crc, &byte| {
        crc ^= byte;
        for _ in 0..8 {
            crc = if crc & 0x80 == 0 {
                crc << 1
            } else {
                (crc << 1) ^ 0x07
            };
        }
        crc
    })
}

fn test_flac_crc16(bytes: &[u8]) -> u16 {
    bytes.iter().fold(0u16, |mut crc, &byte| {
        crc ^= u16::from(byte) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 == 0 {
                crc << 1
            } else {
                (crc << 1) ^ 0x8005
            };
        }
        crc
    })
}

#[test]
fn xadpcm_decodes_with_xbadpcm_and_wraps_exact_pcm_in_flac() {
    use tsfp_import::audio;
    let b = xblock(1000, 0, 0);
    assert!(audio::is_xadpcm(&b, 1));
    let mono_pcm = audio::decode_xadpcm(&b, 1).unwrap();
    assert_eq!(mono_pcm.len(), 64);
    // step 7: nibble 0 adds 7>>3 = 0, so the predictor holds
    assert!(mono_pcm.iter().all(|s| *s == 1000));
    // nibble 7 (+step*13/8) rises, nibble 15 falls, indexes stay clamped in 0..=88
    let up = audio::decode_xadpcm(&xblock(0, 0, 0x77), 1).unwrap();
    assert!(up[1] > 0 && up.windows(2).all(|w| w[1] >= w[0]));
    let down = audio::decode_xadpcm(&xblock(0, 0, 0xFF), 1).unwrap();
    assert!(down[2] < 0);
    // stereo block: two headers, then channel interleaved 4 byte groups
    let mut st = Vec::new();
    st.extend(500i16.to_le_bytes());
    st.extend(0i16.to_le_bytes());
    st.extend((-500i16).to_le_bytes());
    st.extend(0i16.to_le_bytes());
    st.extend([0u8; 64]);
    let stereo_pcm = audio::decode_xadpcm(&st, 2).unwrap();
    assert_eq!(
        (
            stereo_pcm[0],
            stereo_pcm[1],
            stereo_pcm[126],
            stereo_pcm[127]
        ),
        (500, -500, 500, -500)
    );
    let stereo_flac = audio::convert_xadpcm(&st, 2, 22050, "test").unwrap();
    assert_eq!(
        decode_flac(&stereo_flac.flac),
        (
            (22050, 2, 16),
            stereo_pcm.iter().map(|&sample| i32::from(sample)).collect()
        )
    );
    assert!(audio::decode_xadpcm(&xblock(0, 89, 0), 1).is_err());
    assert!(audio::decode_xadpcm(&b[..35], 1).is_err());
    let d = audio::convert_xadpcm(&b, 1, 22050, "t").unwrap();
    assert!(d.flac.starts_with(b"fLaC"));
    assert_eq!(
        decode_flac(&d.flac),
        (
            (22050, 1, 16),
            mono_pcm.iter().map(|&sample| i32::from(sample)).collect()
        )
    );
    assert!(d.sidecar.contains("\"loop\":null") && d.frames == 64);
    let sidecar = d
        .with_sound_key(0xaabbccdd, Some("sounds_x/voice.raw"))
        .sidecar;
    assert!(
        sidecar.contains("\"sound_key\":{\"crc32\":\"aabbccdd\",\"name\":\"sounds_x/voice.raw\"}")
    );
}

#[test]
fn xadpcm_flac_splits_long_pcm_into_valid_frames() {
    use tsfp_import::audio;
    let mut source = Vec::new();
    for block in 0..5 {
        source.extend(xblock(1000 + block, 0, (block as u8) * 0x11));
    }
    let pcm = audio::decode_xadpcm(&source, 1).unwrap();
    let decoded = audio::convert_xadpcm(&source, 1, 32000, "test").unwrap();
    assert_eq!(decoded.frames, 320);
    assert_eq!(
        decode_flac(&decoded.flac),
        (
            (32000, 1, 16),
            pcm.iter().map(|&sample| i32::from(sample)).collect()
        )
    );
}

#[test]
fn riff_xadpcm_keeps_rate_and_loop() {
    use tsfp_import::audio;
    let data = xblock(0, 0, 0);
    let mut fmt = Vec::new();
    for v in [0x69u16, 1] {
        fmt.extend(v.to_le_bytes());
    }
    fmt.extend(32000u32.to_le_bytes());
    fmt.extend(36000u32.to_le_bytes());
    fmt.extend(36u16.to_le_bytes());
    fmt.extend(4u16.to_le_bytes());
    let mut smpl = vec![0u8; 36];
    smpl[28..32].copy_from_slice(&1u32.to_le_bytes());
    smpl.extend([0u8; 8]);
    smpl.extend(10u32.to_le_bytes()); // start
    smpl.extend(49u32.to_le_bytes()); // end inclusive
    smpl.extend([0u8; 8]);
    let mut body = b"WAVE".to_vec();
    for (id, c) in [(&b"fmt "[..], fmt), (b"smpl", smpl), (b"data", data)] {
        body.extend(id);
        body.extend((c.len() as u32).to_le_bytes());
        body.extend(&c);
    }
    let mut f = b"RIFF".to_vec();
    f.extend((body.len() as u32).to_le_bytes());
    f.extend(body);
    let d = audio::convert_riff_xadpcm(&f).unwrap().unwrap();
    assert_eq!((d.rate, d.channels, d.frames), (32000, 1, 64));
    assert!(
        d.sidecar.contains("\"loop\":{\"start\":10,\"end\":50}"),
        "{}",
        d.sidecar
    );
    assert!(audio::convert_riff_xadpcm(b"RIFF\0\0\0\0WAVE").is_none());
}

#[test]
fn vag_decodes_with_loop_flags() {
    use tsfp_import::audio;
    let mut v = b"VAGp".to_vec();
    v.extend([0u8; 12]);
    v.extend(be(11025));
    v.resize(0x30, 0);
    for flag in [4u8, 0, 3, 1] {
        let mut f = vec![0x0C, flag]; // shift 12, filter 0
        f.extend([0x11; 14]);
        v.extend(f);
    }
    let d = audio::convert_vag(&v).unwrap().unwrap();
    assert_eq!((d.rate, d.frames), (11025, 4 * 28));
    assert!(
        d.sidecar.contains("\"loop\":{\"start\":0,\"end\":84}"),
        "{}",
        d.sidecar
    );
    let (spec, samples) = decode_flac(&d.flac);
    assert_eq!(spec, (11025, 1, 16));
    assert_eq!(samples.len(), 4 * 28);
    assert!(audio::convert_vag(b"nope").is_none());
}

#[test]
fn text_bank_sniffing_is_strict() {
    use tsfp_import::convert::sniff_bs;
    let mut b = Vec::new();
    let strs = ["one", "two", "three", "four"];
    let mut off = 16u32;
    for s in strs {
        b.extend(off.to_le_bytes());
        off += s.len() as u32 + 1;
    }
    for s in strs {
        b.extend(s.as_bytes());
        b.push(0);
    }
    assert_eq!(sniff_bs(&b).unwrap().strings.len(), 4);
    assert!(
        sniff_bs(&bank()).is_none(),
        "three strings are not enough evidence"
    );
    assert!(sniff_bs(&xbt(8, 4, 6, 32)).is_none());
    assert!(sniff_bs(&[0u8; 64]).is_none());
    // aliasing offsets over a binary table (seen on real data) are not a bank
    let mut alias = Vec::new();
    for _ in 0..4 {
        alias.extend(16u32.to_le_bytes());
    }
    alias.extend([0u8; 40]);
    assert!(sniff_bs(&alias).is_none());
    use tsfp_import::convert::sniff_plain_text;
    assert!(sniff_plain_text(b"STREAM  a.raw  b.raw  \nFRAMES  12\n"));
    assert!(!sniff_plain_text(b"no line break here at all....") && !sniff_plain_text(b"a\nb\0"));
}

/// A PAK envelope: header, payloads, table of contents at the end of the file.
fn pak(members: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut b = b"P5CK".to_vec();
    b.extend([0u8; 8]);
    let mut toc = Vec::new();
    for (key, body) in members {
        toc.extend(key.to_le_bytes());
        toc.extend((b.len() as u32).to_le_bytes());
        toc.extend((body.len() as u32).to_le_bytes());
        toc.extend(0u32.to_le_bytes());
        b.extend(body);
    }
    let at = b.len() as u32;
    b[4..8].copy_from_slice(&at.to_le_bytes());
    b[8..12].copy_from_slice(&(toc.len() as u32).to_le_bytes());
    b.extend(toc);
    b
}

#[test]
fn sound_bank_members_become_flac_with_named_sidecar_and_text_is_sniffed() {
    let tmp = std::env::temp_dir().join(format!("tsfp-import-snd-{}", std::process::id()));
    let (src, out) = (tmp.join("src"), tmp.join("out"));
    std::fs::create_dir_all(src.join("pak")).unwrap();
    let mut text = Vec::new();
    let strs = ["a", "bb", "ccc", "dddd", "eeeee"];
    let mut off = 20u32;
    for s in strs {
        text.extend(off.to_le_bytes());
        off += s.len() as u32 + 1;
    }
    for s in strs {
        text.extend(s.as_bytes());
        text.push(0);
    }
    let sound_name = "sounds_x/voice.raw";
    let sound_key = tsfp_format_core::crc32(sound_name.as_bytes());
    let mut audio_member = xblock(0, 0, 0x77);
    audio_member.extend(xblock(100, 3, 0x10));
    std::fs::write(
        src.join("pak/sounds_x.pak"),
        pak(&[
            (sound_key, audio_member),
            (0x11223344, text),
            (0x55, vec![9; 40]),
            (0x66, b"STREAM a.raw b.raw\nFRAMES 3\n".to_vec()),
        ]),
    )
    .unwrap();
    let s = source::open(&src, "gc-snd", source::Platform::GameCube).unwrap();
    tsfp_import::convert::stage_a(s.as_ref(), &out, "").unwrap();
    let (o, _) =
        tsfp_import::convert::stage_b_report(s.as_ref(), &out, "", &[sound_name.to_string()])
            .unwrap();
    let ids: Vec<&str> = o.iter().map(|r| r.logical_id.as_str()).collect();
    let audio_flac = format!("audio/pak/sounds_x/{sound_key:08x}.flac");
    let audio_json = format!("audio/pak/sounds_x/{sound_key:08x}.json");
    assert!(ids.contains(&audio_flac.as_str()), "{ids:?}");
    assert!(ids.contains(&audio_json.as_str()));
    assert!(ids.contains(&"text/pak/sounds_x/11223344.json"));
    let unnamed_raw = o
        .iter()
        .find(|r| r.source_path == "pak/sounds_x.pak#00000055" && r.converter == "raw-member")
        .expect("unrecognized PAK member is preserved in the raw area");
    assert_eq!(
        std::fs::read(
            out.join("assets/original/gc-snd")
                .join(&unnamed_raw.logical_id)
        )
        .unwrap(),
        vec![9; 40]
    );
    assert!(ids.contains(&"plain/pak/sounds_x/00000066.txt"), "{ids:?}");
    assert!(ids.iter().any(|id| id.starts_with("raw/")));
    let flac = std::fs::read(out.join("assets/original/gc-snd").join(&audio_flac)).unwrap();
    assert_eq!(decode_flac(&flac).1.len(), 128);
    let sidecar =
        std::fs::read_to_string(out.join("assets/original/gc-snd").join(&audio_json)).unwrap();
    assert!(sidecar.contains(&format!(
        "\"sound_key\":{{\"crc32\":\"{sound_key:08x}\",\"name\":\"{sound_name}\"}}"
    )));
    assert_eq!(tsfp_import::convert::verify(&out, "gc-snd").unwrap(), 7);
    std::fs::remove_dir_all(&tmp).unwrap();
}

#[test]
fn unchanged_stamp_reuses_without_rehashing() {
    let tmp = std::env::temp_dir().join(format!("tsfp-import-stamp-{}", std::process::id()));
    let (src, out) = (tmp.join("src"), tmp.join("out"));
    std::fs::create_dir_all(src.join("data")).unwrap();
    let f = src.join("data/x.bs");
    std::fs::write(&f, bank()).unwrap();
    let s = source::open(&src, "stamp", source::Platform::GameCube).unwrap();
    let run = || {
        tsfp_import::convert::stage_b_report(s.as_ref(), &out, "", &[])
            .unwrap()
            .1
    };
    assert_eq!(run().converted_files, 1);
    assert!(
        out.join("assets/original/stamp/.importer/cache.json")
            .exists()
    );
    assert_eq!(run().reused_files, 1);
    // Same size and mtime but different bytes: the cache trusts the stamp (the documented
    // trade-off), so the file is not re-read.
    let before = std::fs::metadata(&f).unwrap().modified().unwrap();
    let mut same_size = bank();
    same_size[12] = b'z';
    std::fs::write(&f, same_size).unwrap();
    std::fs::File::options()
        .write(true)
        .open(&f)
        .unwrap()
        .set_modified(before)
        .unwrap();
    assert_eq!(run().reused_files, 1);
    // A new mtime forces a re-read and a re-hash.
    bump_mtime(&f);
    assert_eq!(run().converted_files, 1);
    std::fs::remove_dir_all(&tmp).unwrap();
}

/// Real-data check, skipped unless TSFP_PS2_ISO names the owner's PS2 image.
#[test]
fn real_ps2_image_pak_and_vag() {
    let Ok(iso) = std::env::var("TSFP_PS2_ISO") else {
        eprintln!("skip: TSFP_PS2_ISO unset");
        return;
    };
    let s = source::open(std::path::Path::new(&iso), "ps2-usa", source::Platform::Ps2).unwrap();
    let files = s.list().unwrap();
    let f = files
        .iter()
        .find(|f| f.path == "PAK/ANIM/ANIM_MG.PAK")
        .unwrap();
    let data = s.read(f).unwrap();
    let p = tsfp_format_pak::Pak::parse(&data).unwrap();
    assert!(!p.entries.is_empty(), "PS2 PAKs use the same P5CK envelope");
}

/// Real-data check, skipped unless TSFP_GCM names an uncompressed GameCube image.
#[test]
fn real_gcm_image_lists_files() {
    let Ok(iso) = std::env::var("TSFP_GCM") else {
        eprintln!("skip: TSFP_GCM unset");
        return;
    };
    let s = source::open(
        std::path::Path::new(&iso),
        "gc-usa",
        source::Platform::GameCube,
    )
    .unwrap();
    let files = s.list().unwrap();
    assert!(files.iter().any(|f| f.path == "sys/main.dol"));
    assert!(files.len() > 10);
}
