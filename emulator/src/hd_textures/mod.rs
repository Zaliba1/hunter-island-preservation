// SPDX-License-Identifier: MPL-2.0
//! Optional, local Hunter Island monster texture cache. Guest sprite geometry
//! and the contents of the imported IPA are unchanged.
mod model;
#[cfg(test)]
mod tests;

use crate::image::Image;
use flate2::{read::ZlibDecoder, write::ZlibEncoder, Compression};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, SyncSender};
use zip::ZipArchive;

const SCALE: usize = 4;
const MAX_SIDE: usize = 1536;
const MAX_PIXELS: usize = 1024 * 1024;
const MAX_PNG: u64 = 6 * 1024 * 1024;

struct Sprite {
    index: usize,
    key: String,
    // Opaque points survive the game's straight/premultiplied conversions.
    probes: Vec<(usize, [u8; 3])>,
}

struct Job {
    key: String,
    size: (usize, usize),
    pixels: Vec<u8>,
}

pub struct Cache {
    archive: ZipArchive<File>,
    sprites: HashMap<(usize,usize), Vec<Sprite>>,
    directory: PathBuf,
    jobs: SyncSender<Job>,
    done: Receiver<String>,
    pending: HashSet<String>,
}

fn monster(name: &str) -> bool {
    let Some(base) = name.strip_suffix("-hd.png") else { return false };
    if ["Arborus","Dahag","Esapid","Noah","Rakkengar","Thravezzat","Tiamat"].contains(&base) {
        return true
    }
    let bytes = base.as_bytes();
    bytes.len()>2 && !base.starts_with("TournamentNPC") &&
        bytes[..bytes.len()-2].iter().all(u8::is_ascii_alphabetic) &&
        bytes[bytes.len()-2..].iter().all(u8::is_ascii_digit)
}

fn invalid() -> io::Error { io::Error::new(io::ErrorKind::InvalidData,"Invalid texture data") }

fn png(archive: &mut ZipArchive<File>, index: usize) -> io::Result<Vec<u8>> {
    let mut member = archive.by_index(index)?;
    if member.size()>MAX_PNG {return Err(invalid())}
    let mut data = Vec::new();
    member.by_ref().take(MAX_PNG+1).read_to_end(&mut data)?;
    if data.len()<33 || data.len() as u64>MAX_PNG || !data.starts_with(b"\x89PNG\r\n\x1a\n") || &data[12..16]!=b"IHDR" {return Err(invalid())}
    let w = u32::from_be_bytes(data[16..20].try_into().unwrap()) as usize;
    let h = u32::from_be_bytes(data[20..24].try_into().unwrap()) as usize;
    if w==0 || h==0 || w>MAX_SIDE || h>MAX_SIDE || w*h>MAX_PIXELS {return Err(invalid())}
    Ok(data)
}

fn probes(image: &Image) -> Vec<(usize,[u8;3])> {
    let opaque = image.pixels().chunks_exact(4).filter(|p|p[3]==255).count();
    let step = (opaque/96).max(1);
    image.pixels().chunks_exact(4).enumerate().filter(|(_,p)|p[3]==255)
        .step_by(step).take(96).map(|(i,p)|(i,[p[0],p[1],p[2]])).collect()
}

fn read_cache(path: &Path, len: usize) -> io::Result<Vec<u8>> {
    let file = File::open(path)?;
    if file.metadata()?.len()>len as u64+65536 {return Err(invalid())}
    let mut data = Vec::new();
    ZlibDecoder::new(file).take((len+1) as u64).read_to_end(&mut data)?;
    if data.len()!=len {return Err(invalid())}
    Ok(data)
}

fn write_cache(path: &Path, data: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension(format!("{}.tmp",uuid::Uuid::new_v4()));
    let result = (|| {
        let mut enc = ZlibEncoder::new(File::create_new(&tmp)?,Compression::fast());
        enc.write_all(data)?;
        enc.finish()?.sync_all()?;
        fs::rename(&tmp,path)
    })();
    if result.is_err() {let _=fs::remove_file(tmp);}
    result
}

impl Cache {
    pub fn open(ipa: &Path, directory: PathBuf) -> io::Result<Self> {
        let mut archive = ZipArchive::new(File::open(ipa)?)?;
        let mut sprites: HashMap<_,Vec<_>> = HashMap::new();
        for index in 0..archive.len() {
            let entry = archive.by_index(index)?;
            let mut parts = entry.name().split('/');
            if parts.next()!=Some("Payload") || !parts.next().is_some_and(|p|p.ends_with(".app")) ||
                !parts.next().is_some_and(monster) || parts.next().is_some() { continue }
            drop(entry);
            let Ok(data) = png(&mut archive,index) else { continue };
            let Ok(img) = Image::from_bytes(&data) else { continue };
            let probes = probes(&img);
            if probes.len()<16 {continue}
            let (w,h) = img.dimensions();
            sprites.entry((w as usize,h as usize)).or_default().push(Sprite {
                index, key: format!("{:x}",Sha256::digest(&data)), probes
            });
        }
        fs::create_dir_all(&directory)?;
        if fs::symlink_metadata(&directory)?.file_type().is_symlink() {return Err(invalid())}
        let (jobs,queue) = mpsc::sync_channel::<Job>(3);
        let (finished,done) = mpsc::channel();
        let dir = directory.clone();
        std::thread::Builder::new().name("local-hd-textures".into()).spawn(move || {
            let mut model = None;
            while let Ok(job) = queue.recv() {
                let path = dir.join(&job.key);
                let (w,h) = job.size;
                let hd = model.get_or_insert_with(model::Model::load).upscale(&job.pixels,w,h);
                match write_cache(&path,&hd) {
                    Ok(()) => { log!("Local HD texture cached: {w}x{h} -> {}x{}",w*SCALE,h*SCALE); }
                    Err(e) => { log!("Local HD texture could not be cached: {e}"); }
                }
                if finished.send(job.key).is_err() {break}
            }
        })?;
        log!("Local Real-ESRGAN: indexed {} monster images",sprites.values().map(Vec::len).sum::<usize>());
        Ok(Self {archive,sprites,directory,jobs,done,pending:HashSet::new()})
    }

    pub fn has_size(&self,w:i32,h:i32) -> bool { self.sprites.contains_key(&(w as usize,h as usize)) }

    pub fn replacement(&mut self, size: (usize,usize), raw: &[u8], stride: usize, format: u32, ty: u32) -> Option<Vec<u8>> {
        for key in self.done.try_iter() {self.pending.remove(&key);}
        let (w,h) = size;
        let candidates = self.sprites.get(&size)?;
        let pixel = |i:usize| -> [u8;4] {
            let row = (i/w)*stride;
            let x = i%w;
            if ty==0x8033 {
                let p = row+x*2;
                let v = u16::from_ne_bytes([raw[p],raw[p+1]]);
                return [12,8,4,0].map(|s|(((v>>s)&15)*17) as u8)
            }
            let p = &raw[row+x*4..][..4];
            if format==0x80e1 {[p[2],p[1],p[0],p[3]]} else {[p[0],p[1],p[2],p[3]]}
        };
        let sprite = candidates.iter().find(|s| s.probes.iter().all(|(i,rgb)| {
            let p = pixel(*i);
            p[3]>=240 && (0..3).all(|c|p[c].abs_diff(rgb[c])<=17)
        }))?;
        let path = self.directory.join(&sprite.key);
        if let Ok(bytes) = read_cache(&path,w*h*SCALE*SCALE*4) {return Some(bytes)}
        if self.pending.contains(&sprite.key) {return None}
        let data = png(&mut self.archive,sprite.index).ok()?;
        let img = Image::from_bytes(&data).ok()?;
        let job = Job {key:sprite.key.clone(),size,pixels:img.pixels().to_vec()};
        if self.jobs.try_send(job).is_ok() {self.pending.insert(sprite.key.clone());}
        None
    }
}
