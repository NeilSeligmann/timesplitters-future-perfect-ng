//! Explicit inspection tool. `dump` emits private payload only when requested.
use tsfp_data_formats::*;
fn run()->Result<(),String>{
 let a:Vec<_>=std::env::args().collect();if a.len()<4{return Err("usage: tsfp-formats list|dump pak|xbr|xbt|bs|csinfo|igcs|profile|mkr|setup|rule|point FILE [count or entry index]".into());}
 let b=std::fs::read(&a[3]).map_err(|e|e.to_string())?;
 let number=||a.get(4).ok_or("count/index required").and_then(|s|s.parse::<usize>().map_err(|_|"invalid number"));
 let dump=a[1]=="dump";if !dump&&a[1]!="list"{return Err("expected list or dump".into());}
 let out:Vec<u8>=match a[2].as_str(){
 "pak"=>{let p=pak::Pak::parse(&b)?;if dump{p.payload(number()?)?.to_vec()}else{for(i,e)in p.entries.iter().enumerate(){println!("{i}: key={:08x} offset={} decoded={} stored={}",e.key,e.offset,e.decoded_size,e.stored_size);}Vec::new()}},
 "bs"=>{let p=bs::Bank::parse(&b,number()?)?;if dump{p.encode()?}else{for i in 0..p.offsets.len(){println!("{i}: offset={} bytes={}",p.offsets[i],p.string(i)?.len());}Vec::new()}},
 "xbr"=>{let p=xbr::Level::parse(&b)?;if !dump{println!("rooms={}",p.rooms.len());}if dump{p.encode()?}else{Vec::new()}},
 "xbt"=>{let p=xbt::Texture::parse(&b)?;if !dump{println!("dimensions={}x{} format={} payload={}",p.words[0],p.words[1],p.words[5],p.pixels.len());}if dump{p.encode()?}else{Vec::new()}},
 "csinfo"=>{let p=csinfo::Csinfo::parse(&b)?;if !dump{for r in &p.records{println!("{}: {} arguments",r.directive,r.args.len());}}if dump{p.encode()?}else{Vec::new()}},
 "igcs"=>{let p=igcs::Animation::parse(&b)?;if !dump{println!("nodes={} frames={} blocks={}",p.nodes.len(),p.header[4],p.header[6]);}if dump{p.encode()?}else{Vec::new()}},
 "profile"=>{let p=profile::Profile::parse(&b)?;if !dump{println!("version={} payload={} signature=20 (not authenticated)",p.version()?,p.payload.len());}if dump{p.encode()?}else{Vec::new()}},
 "mkr"=>{let p=script::mkr::Mkr::parse(&b)?;if !dump{for c in &p.chunks{println!("tag={:08x} bytes={}",c.tag,c.payload.len());}println!("setups={} rules={} tail={}",p.setups()?.len(),p.rules()?.len(),p.tail.len());}if dump{p.encode()?}else{Vec::new()}},
 "setup"|"rule"|"point"=>{let k=match a[2].as_str(){"setup"=>script::Kind::Setup,"rule"=>script::Kind::Rule,_=>script::Kind::Point};let p=script::Records::parse(&b,k)?;if !dump{println!("records={} stride={}",p.records.len(),k.stride());}if dump{p.encode()?}else{Vec::new()}},_=>return Err("unsupported format".into())};
 if dump{use std::io::Write;std::io::stdout().write_all(&out).map_err(|e|e.to_string())?;}Ok(())}
fn main(){if let Err(e)=run(){eprintln!("{e}");std::process::exit(1);}}
