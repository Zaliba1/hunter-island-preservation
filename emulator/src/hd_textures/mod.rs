// SPDX-License-Identifier: MPL-2.0
//! Optional, local Hunter Island monster texture cache. Guest sprite geometry
//! and the contents of the imported IPA are unchanged.
mod artwork;
mod model;
use artwork::Artwork;
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

const MAX_SIDE: usize = 2048;
const MAX_PIXELS: usize = 1024 * 1024;
const MAX_MEMBER: u64 = 6 * 1024 * 1024;

struct Sprite {
    artwork: Artwork,
    index: usize,
    key: String,
    // Opaque points survive the game's straight/premultiplied conversions.
    probes: Vec<(usize, [u8; 3])>,
}

struct Job {
    artwork: Artwork,
    key: String,
    size: (usize, usize),
    pixels: Vec<u8>,
}

pub struct Cache {
    archive: ZipArchive<File>,
    sprites: HashMap<(usize, usize), Vec<Sprite>>,
    directory: PathBuf,
    jobs: SyncSender<Job>,
    done: Receiver<String>,
    pending: HashSet<String>,
}

fn monster(name: &str) -> bool {
    let Some(base) = name.strip_suffix("-hd.png") else {
        return false;
    };
    if [
        "Arborus",
        "Dahag",
        "Esapid",
        "Noah",
        "Rakkengar",
        "Thravezzat",
        "Tiamat",
    ]
    .contains(&base)
    {
        return true;
    }
    let bytes = base.as_bytes();
    bytes.len() > 2
        && !base.starts_with("TournamentNPC")
        && bytes[..bytes.len() - 2].iter().all(u8::is_ascii_alphabetic)
        && bytes[bytes.len() - 2..].iter().all(u8::is_ascii_digit)
}

fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "Invalid texture data")
}

fn image_data(archive: &mut ZipArchive<File>, index: usize) -> io::Result<Vec<u8>> {
    let mut member = archive.by_index(index)?;
    if member.size() > MAX_MEMBER {
        return Err(invalid());
    }
    let mut data = Vec::new();
    member
        .by_ref()
        .take(MAX_MEMBER + 1)
        .read_to_end(&mut data)?;
    if data.len() as u64 > MAX_MEMBER {
        return Err(invalid());
    }
    let (w, h) = artwork::dimensions(&data).ok_or_else(invalid)?;
    if w == 0 || h == 0 || w > MAX_SIDE || h > MAX_SIDE || w * h > MAX_PIXELS {
        return Err(invalid());
    }
    Ok(data)
}

fn probes(image: &Image) -> Vec<(usize, [u8; 3])> {
    let opaque = image
        .pixels()
        .chunks_exact(4)
        .filter(|p| p[3] == 255)
        .count();
    let step = (opaque / 96).max(1);
    image
        .pixels()
        .chunks_exact(4)
        .enumerate()
        .filter(|(_, p)| p[3] == 255)
        .step_by(step)
        .take(96)
        .map(|(i, p)| (i, [p[0], p[1], p[2]]))
        .collect()
}

fn read_cache(path: &Path, len: usize) -> io::Result<Vec<u8>> {
    let file = File::open(path)?;
    if file.metadata()?.len() > len as u64 + 65536 {
        return Err(invalid());
    }
    let mut data = Vec::new();
    ZlibDecoder::new(file)
        .take((len + 1) as u64)
        .read_to_end(&mut data)?;
    if data.len() != len {
        return Err(invalid());
    }
    Ok(data)
}

fn write_cache(path: &Path, data: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut enc = ZlibEncoder::new(File::create_new(&tmp)?, Compression::fast());
        enc.write_all(data)?;
        enc.finish()?.sync_all()?;
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(tmp);
    }
    result
}

impl Cache {
    pub fn open(ipa: &Path, directory: PathBuf) -> io::Result<Self> {
        let mut archive = ZipArchive::new(File::open(ipa)?)?;
        let mut sprites: HashMap<_, Vec<_>> = HashMap::new();
        for index in 0..archive.len() {
            let entry = archive.by_index(index)?;
            let path = entry.name().to_owned();
            drop(entry);
            let mut parts = path.split('/');
            if parts.next() != Some("Payload") || !parts.next().is_some_and(|p| p.ends_with(".app"))
            {
                continue;
            }
            let Some(name) = parts.next() else { continue };
            if parts.next().is_some() {
                continue;
            }
            let is_atlas = matches!(name, "MonsterPortraits-hd.png" | "HeroWalkFrames-hd.png");
            let kind = match name {
                "Tavern.jpg" | "CourtRoom.jpg" | "StoneHouse.jpg" | "CastleCombat.jpg" => {
                    Artwork::Painted
                }
                "TournamentNPC1-hd.png" => Artwork::Portrait,
                n if monster(n) => Artwork::Monster,
                _ if is_atlas => Artwork::Portrait, // replaced after validating the local frame table
                _ => continue,
            };
            let Ok(data) = image_data(&mut archive, index) else {
                continue;
            };
            let (w, h) = artwork::dimensions(&data).unwrap();
            let mut digest = Sha256::new();
            digest.update(&data);
            let kind = if is_atlas {
                let Ok(Artwork::Atlas(frames)) = artwork::atlas(&mut archive, &path, w, h) else {
                    continue;
                };
                for rect in frames.iter() {
                    for n in rect {
                        digest.update((*n as u32).to_be_bytes());
                    }
                }
                Artwork::Atlas(frames)
            } else {
                kind
            };
            let key = match kind {
                Artwork::Monster => format!("{:x}", digest.finalize()),
                _ => format!("art2-v1-{:x}", digest.finalize()),
            };
            let Ok(img) = Image::from_bytes(&data) else {
                continue;
            };
            let probes = probes(&img);
            if probes.len() < 16 {
                continue;
            }
            let (w, h) = img.dimensions();
            sprites
                .entry((w as usize, h as usize))
                .or_default()
                .push(Sprite {
                    index,
                    key,
                    artwork: kind,
                    probes,
                });
        }
        fs::create_dir_all(&directory)?;
        if fs::symlink_metadata(&directory)?.file_type().is_symlink() {
            return Err(invalid());
        }
        let (jobs, queue) = mpsc::sync_channel::<Job>(3);
        let (finished, done) = mpsc::channel();
        let dir = directory.clone();
        std::thread::Builder::new()
            .name("local-hd-textures".into())
            .spawn(move || {
                let mut model = None;
                while let Ok(job) = queue.recv() {
                    let path = dir.join(&job.key);
                    let (w, h) = job.size;
                    let scale = job.artwork.scale();
                    let hd = job.artwork.render(
                        model.get_or_insert_with(model::Model::load),
                        &job.pixels,
                        w,
                        h,
                    );
                    match write_cache(&path, &hd) {
                        Ok(()) => {
                            log!(
                                "Local HD texture cached: {w}x{h} -> {}x{}",
                                w * scale,
                                h * scale
                            );
                        }
                        Err(e) => {
                            log!("Local HD texture could not be cached: {e}");
                        }
                    }
                    if finished.send(job.key).is_err() {
                        break;
                    }
                }
            })?;
        log!(
            "Local Real-ESRGAN: indexed {} artwork textures",
            sprites.values().map(Vec::len).sum::<usize>()
        );
        Ok(Self {
            archive,
            sprites,
            directory,
            jobs,
            done,
            pending: HashSet::new(),
        })
    }

    pub fn has_size(&self, w: i32, h: i32) -> bool {
        self.sprites.contains_key(&(w as usize, h as usize))
    }

    pub fn replacement(
        &mut self,
        size: (usize, usize),
        raw: &[u8],
        stride: usize,
        format: u32,
        ty: u32,
        maximum: usize,
    ) -> Option<(usize, Vec<u8>)> {
        for key in self.done.try_iter() {
            self.pending.remove(&key);
        }
        let (w, h) = size;
        let candidates = self.sprites.get(&size)?;
        let pixel = |i: usize| -> [u8; 4] {
            let row = (i / w) * stride;
            let x = i % w;
            if ty == 0x8033 {
                let p = row + x * 2;
                let v = u16::from_ne_bytes([raw[p], raw[p + 1]]);
                return [12, 8, 4, 0].map(|s| (((v >> s) & 15) * 17) as u8);
            }
            if ty == 0x8363 {
                let i = row + x * 2;
                let v = u16::from_ne_bytes([raw[i], raw[i + 1]]);
                return [
                    ((v >> 11) * 255 / 31) as u8,
                    (((v >> 5) & 63) * 255 / 63) as u8,
                    ((v & 31) * 255 / 31) as u8,
                    255,
                ];
            }
            if format == 0x1907 {
                let i = row + x * 3;
                return [raw[i], raw[i + 1], raw[i + 2], 255];
            }
            let p = &raw[row + x * 4..][..4];
            if format == 0x80e1 {
                [p[2], p[1], p[0], p[3]]
            } else {
                [p[0], p[1], p[2], p[3]]
            }
        };
        // Samples cheaply reject unrelated UI and atlas uploads. Verify the
        // full silhouette and color before substituting any texture.
        let (sprite, img) = candidates.iter().find_map(|s| {
            if w * s.artwork.scale() > maximum || h * s.artwork.scale() > maximum {
                return None;
            }
            if !s.probes.iter().all(|(i, rgb)| {
                let p = pixel(*i);
                p[3] >= 240 && (0..3).all(|c| p[c].abs_diff(rgb[c]) <= 17)
            }) {
                return None;
            }
            let data = image_data(&mut self.archive, s.index).ok()?;
            let img = Image::from_bytes(&data).ok()?;
            let same = img
                .pixels()
                .chunks_exact(4)
                .enumerate()
                .all(|(i, original)| {
                    let p = pixel(i);
                    original[3].abs_diff(p[3]) <= 17
                        && (0..3).all(|c| {
                            let premult = ((u16::from(p[c]) * u16::from(p[3]) + 127) / 255) as u8;
                            original[c].abs_diff(p[c]) <= 17 || original[c].abs_diff(premult) <= 17
                        })
                });
            same.then_some((s, img))
        })?;
        let scale = sprite.artwork.scale();
        let path = self.directory.join(&sprite.key);
        if let Ok(bytes) = read_cache(&path, w * h * scale * scale * 4) {
            return Some((scale, bytes));
        }
        if self.pending.contains(&sprite.key) {
            return None;
        }
        let job = Job {
            artwork: sprite.artwork.clone(),
            key: sprite.key.clone(),
            size,
            pixels: img.pixels().to_vec(),
        };
        if self.jobs.try_send(job).is_ok() {
            self.pending.insert(sprite.key.clone());
        }
        None
    }
}
