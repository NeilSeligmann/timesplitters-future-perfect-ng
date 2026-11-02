use tsfp_data_formats::*;
#[test] fn short_inputs_never_panic() {
 for n in 0..256 {let b=vec![0;n];assert!(pak::Pak::parse(&b).is_err());assert!(xbr::Level::parse(&b).is_err());assert!(xbt::Texture::parse(&b).is_err());assert!(igcs::Animation::parse(&b).is_err());assert!(profile::Profile::parse(&b).is_err());assert!(csinfo::Csinfo::parse(&b).is_err());if n<40{assert!(bs::Bank::parse(&b,10).is_err());}}
}
#[test] fn typed_edits_change_serialized_bytes() {
 let mut b=vec![0;128];for o in [0,4,8,12]{core::put(&mut b,o,4);}let mut t=xbt::Texture::parse(&b).unwrap();t.words[7]=0x7fc12345;let e=t.encode().unwrap();assert_eq!(core::word(&e,28).unwrap(),0x7fc12345);assert_eq!(&e[36..],&b[36..]);
 let mut b=vec![0;0x1f68];core::put(&mut b,0x1ee0,2);let mut p=profile::Profile::parse(&b).unwrap();p.payload[4]=99;let e=p.encode().unwrap();assert_eq!(e[4],99);assert_eq!(&e[0x1f54..],&b[0x1f54..]);
 let mut b=vec![0;0xb4];b[12..14].copy_from_slice(&(-1i16).to_le_bytes());let mut s=script::Records::parse(&b,script::Kind::Setup).unwrap();assert_eq!(s.signed_index(0).unwrap(),-1);s.records[0][0]=2;assert_eq!(s.encode().unwrap()[0],2);
}
#[test] fn bad_mutations_are_errors() {
 let mut b=vec![0;32];b[..4].copy_from_slice(b"P5CK");core::put(&mut b,4,16);core::put(&mut b,8,16);core::put(&mut b,20,12);core::put(&mut b,24,4);let mut p=pak::Pak::parse(&b).unwrap();p.entries.push(p.entries[0].clone());assert!(p.encode().is_err());p.entries.pop();p.entries[0].offset=u32::MAX;assert!(p.encode().is_err());
 let mut c=csinfo::Csinfo::parse(b"PROP a").unwrap();c.records[0].args[0]="bad token".into();assert!(c.encode().is_err());
}
#[test] fn deterministic_malformed_inputs() {
 let mut seed=0x1234u32;for len in 0..1024{let mut b=vec![0;len];for byte in &mut b{seed=seed.wrapping_mul(1664525).wrapping_add(1013904223);*byte=(seed>>24)as u8;}let _=pak::Pak::parse(&b);let _=xbr::Level::parse(&b);let _=xbt::Texture::parse(&b);let _=igcs::Animation::parse(&b);let _=csinfo::Csinfo::parse(&b);let _=bs::Bank::parse(&b,len/4);let _=profile::Profile::parse(&b);for k in [script::Kind::Setup,script::Kind::Rule,script::Kind::Point]{let _=script::Records::parse(&b,k);}}
}
