//! Owner-disc asset-load checks (T2047, Tier 1 on real data). Skipped unless TSFP_DISC_ISO
//! names the image; prints counts only, never names or bytes.
use tsfp_engine_fileio::assets::{AssetKey, AssetLoader};
use tsfp_engine_fileio::pak::PakIndex;
use tsfp_engine_fileio::vfs::{Access, Disposition, Vfs};
use tsfp_engine_fileio::xiso::Xiso;

const REFERENCE_LIMIT: u64 = 64 << 20;

/// Every entry of every PAK decodes through the streaming loader exactly as the format crate
/// decodes it, and XBT/XBR envelope parsing agrees on success or failure for each entry.
#[test]
fn every_pak_entry_payload_and_envelope_matches_the_format_crates() {
    let Some(iso) = std::env::var_os("TSFP_DISC_ISO") else {
        eprintln!("SKIP: TSFP_DISC_ISO unset");
        return;
    };
    let disc = Xiso::open(std::path::Path::new(&iso)).unwrap();
    let paks: Vec<_> = disc
        .walk()
        .unwrap()
        .into_iter()
        .filter(|(p, _)| p.to_ascii_lowercase().ends_with(".pak"))
        .collect();
    let mut vfs = Vfs::new();
    vfs.mount_disc("\\Device\\CdRom0", disc);
    vfs.add_symlink("D:", "\\Device\\CdRom0");
    let mut loader = AssetLoader::new(vfs);
    let (mut compared, mut streamed, mut textures, mut levels, mut mismatched) =
        (0usize, 0usize, 0usize, 0usize, 0usize);
    for (path, file) in &paks {
        let guest = format!("D:\\{path}");
        let vfs = loader.vfs();
        let h = vfs.open(&guest, Access::READ, Disposition::Open).unwrap();
        let index = PakIndex::open(vfs, h).unwrap();
        let reference = if u64::from(file.size) <= REFERENCE_LIMIT {
            let mut whole = vec![0u8; file.size as usize];
            assert_eq!(vfs.read_at(h, 0, &mut whole).unwrap(), whole.len());
            Some(whole)
        } else {
            None
        };
        let entries = index.entries.clone();
        let mut payloads = Vec::new();
        for i in 0..entries.len() {
            payloads.push(index.payload(vfs, i).ok());
        }
        vfs.close(h).unwrap();
        let parsed = reference
            .as_deref()
            .map(|b| tsfp_format_pak::Pak::parse(b).unwrap());
        for (i, entry) in entries.iter().enumerate() {
            let payload = payloads[i].as_deref();
            if let Some(pak) = &parsed {
                compared += 1;
                if payload != pak.payload(i).ok() {
                    mismatched += 1;
                    continue;
                }
            } else {
                streamed += 1;
            }
            let Some(bytes) = payload else { continue };
            let key = AssetKey {
                key: entry.key,
                occurrence: entries[..i].iter().filter(|e| e.key == entry.key).count(),
            };
            let tex_ref = tsfp_format_xbt::Texture::parse(bytes).is_ok();
            let lvl_ref = tsfp_format_xbr::Level::parse(bytes).is_ok();
            if tex_ref != loader.texture(&guest, key).is_ok() {
                mismatched += 1;
            }
            if lvl_ref != loader.level(&guest, key).is_ok() {
                mismatched += 1;
            }
            textures += tex_ref as usize;
            levels += lvl_ref as usize;
        }
    }
    assert_eq!(loader.vfs().open_count(), 0, "loader leaked handles");
    eprintln!(
        "paks={} compared={compared} streamed_only={streamed} xbt_ok={textures} xbr_ok={levels} mismatched={mismatched}",
        paks.len()
    );
    assert_eq!(mismatched, 0);
}

/// The first playable Story row's asset (the host's `first_story_asset_selection`) loads through
/// the loader path and decodes to an XBR level envelope.
#[test]
fn first_story_row_asset_loads_as_an_xbr_level() {
    let Some(iso) = std::env::var_os("TSFP_DISC_ISO") else {
        eprintln!("SKIP: TSFP_DISC_ISO unset");
        return;
    };
    let mut vfs = Vfs::new();
    vfs.mount_disc("D:", Xiso::open(std::path::Path::new(&iso)).unwrap());
    let mut loader = AssetLoader::new(vfs);
    let selected = AssetKey {
        key: 0x4afe_3652,
        occurrence: 0,
    };
    let level = loader.level("D:\\pak\\story\\l_2_st.pak", selected);
    eprintln!("first story row xbr level ok={}", level.is_ok());
    assert!(level.is_ok());
    assert!(
        loader
            .level(
                "D:\\pak\\story\\l_2_st.pak",
                AssetKey {
                    key: 0x4afe_3652,
                    occurrence: 9999
                }
            )
            .is_err()
    );
    assert_eq!(loader.vfs().open_count(), 0);
}
