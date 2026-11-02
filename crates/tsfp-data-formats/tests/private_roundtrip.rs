use tsfp_data_formats::*;
fn sample(kind: &str) -> Option<Vec<u8>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = std::process::Command::new(std::env::var("TSFP_TEST_PYTHON").unwrap_or("python3".into()))
        .current_dir(root).args(["-m", "tools.rust_format_sample", kind]).output().expect("Python sample reader");
    if output.status.code() == Some(77) { eprintln!("SKIP {kind}: private ISO absent"); return None; }
    assert!(output.status.success(), "private reader failed: {}", String::from_utf8_lossy(&output.stderr));
    Some(output.stdout)
}
#[test] fn disc_envelopes() {
    for kind in ["pak", "xbr", "xbt", "bs", "bs-story", "csinfo", "igcs"] {
        let Some(b) = sample(kind) else { continue };
        let encoded = match kind {
            "pak" => pak::Pak::parse(&b).unwrap().encode().unwrap(),
            "xbr" => {let x=xbr::Level::parse(&b).unwrap();assert_eq!(x.rooms.len(),51);x.encode().unwrap()},
            "xbt" => xbt::Texture::parse(&b).unwrap().encode().unwrap(),
            "bs" | "bs-story" => bs::Bank::parse(&b, if kind=="bs"{3463}else{4149}).unwrap().encode().unwrap(),
            "csinfo" => {let c=csinfo::Csinfo::parse(&b).unwrap(); let e=c.encode().unwrap(); assert_eq!(csinfo::Csinfo::parse(&e).unwrap().records,c.records);eprintln!("MEASURED {kind}: {} bytes, semantic round trip",b.len()); continue},
            "igcs" => igcs::Animation::parse(&b).unwrap().encode().unwrap(),
            _ => unreachable!(),
        };
        assert!(encoded==b,"{kind} differs");
        eprintln!("MEASURED {kind}: {} bytes, byte-identical",b.len());
    }
}
#[test] fn private_profiles() {
    let root=std::path::PathBuf::from(std::env::var("TSFP_TEST_HDD").unwrap_or("/workspace/tmp/hdds/profile-hdd".into()));
    if !root.exists(){eprintln!("SKIP profile: private HDD absent");return;}
    fn walk(p:&std::path::Path,n:&mut usize){for e in std::fs::read_dir(p).unwrap(){let e=e.unwrap();let p=e.path();if e.file_type().unwrap().is_dir(){walk(&p,n);}else if e.file_name()=="tsftrpft"{let b=std::fs::read(p).unwrap();assert!(profile::Profile::parse(&b).unwrap().encode().unwrap()==b);*n+=1;}}}
    let mut n=0;walk(&root,&mut n);assert!(n>0,"private HDD present but profile witness absent");eprintln!("MEASURED profile: {n} byte-identical files");
}
#[test] fn private_mapmaker_script_records() {
    let root=std::path::PathBuf::from(std::env::var("TSFP_TEST_HDD").unwrap_or("/workspace/tmp/hdds/editor-hdd".into()));
    if !root.exists(){eprintln!("SKIP mkr: private HDD absent");return;}
    fn walk(p:&std::path::Path,n:&mut usize,setups:&mut usize){for e in std::fs::read_dir(p).unwrap(){let e=e.unwrap();let p=e.path();if e.file_type().unwrap().is_dir(){walk(&p,n,setups);}else if p.extension().is_some_and(|x|x=="mkr"){let b=std::fs::read(p).unwrap();let m=script::mkr::Mkr::parse(&b).unwrap();assert!(m.encode().unwrap()==b);*setups+=m.setups().unwrap().len();m.rules().unwrap();*n+=1;}}}
    let(mut n,mut setups)=(0,0);walk(&root,&mut n,&mut setups);
    assert!(n>0,"private HDD present but .mkr witness absent");assert!(setups>0,"no real setup record in the .mkr witness");
    eprintln!("MEASURED mkr: {n} byte-identical files, {setups} real 0xB4 setup records");
}
