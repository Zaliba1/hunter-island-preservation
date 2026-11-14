/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Diagnostic-only elapsed-time aggregation at guest CPU return boundaries.
//! Budget-end PCs are coarse samples, not native-IP maps or per-function times.
use std::collections::HashMap;
use std::time::Duration;

const LIMIT: usize = 8192;
use std::sync::atomic::{AtomicU64, Ordering};
static IMAGE_FAST: AtomicU64 = AtomicU64::new(0);
static IMAGE_FAST_PIXELS: AtomicU64 = AtomicU64::new(0);
static IMAGE_FALLBACK: AtomicU64 = AtomicU64::new(0);
static LEAF_GETTERS: AtomicU64 = AtomicU64::new(0);
pub(crate) fn record_leaf_getter() {
    LEAF_GETTERS.fetch_add(1, Ordering::Relaxed);
}
pub(crate) fn record_image(pixels: u64, fast: bool) {
    if fast {
        IMAGE_FAST.fetch_add(1, Ordering::Relaxed);
        IMAGE_FAST_PIXELS.fetch_add(pixels, Ordering::Relaxed);
    } else {
        IMAGE_FALLBACK.fetch_add(1, Ordering::Relaxed);
    }
}

pub(crate) fn dump_symbols(bins: &[crate::mach_o::MachO]) -> Result<(), String> {
    use std::io::Write;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    std::fs::create_dir_all("temp").map_err(|e| e.to_string())?;
    let path = format!("temp/guest-symbols-{}-{stamp}.tsv", std::process::id());
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    let mut output = std::io::BufWriter::new(file);
    let mut rows = 0;
    for bin in bins {
        for section in &bin.sections {
            writeln!(
                output,
                "section\t{}\t{:#x}\t{:#x}\t{}",
                bin.name, section.addr, section.size, section.name
            )
            .map_err(|e| e.to_string())?;
        }
        for (name, addr) in &bin.exported_symbols {
            if rows >= 250_000 {
                return Err("Diagnostic symbol limit reached".into());
            }
            let name = name.replace('\t', "\\t").replace('\n', "\\n");
            writeln!(output, "symbol\t{}\t{addr:#x}\t{name}", bin.name)
                .map_err(|e| e.to_string())?;
            rows += 1;
        }
    }
    output.flush().map_err(|e| e.to_string())?;
    log!(
        "GUEST_PROFILE_SYMBOLS: file={path} modules={} symbols={rows}",
        bins.len()
    );
    Ok(())
}
#[derive(Clone, Copy, Default)]
struct Count {
    calls: u64,
    ns: u128,
}
#[derive(Default)]
pub(super) struct Profile {
    rows: HashMap<(bool, u32, u32, bool), Count>,
    calls: u64,
    ns: u128,
    overflow_calls: u64,
    overflow_ns: u128,
}
impl Profile {
    pub fn record(
        &mut self,
        pc: u32,
        lr: u32,
        thumb: bool,
        result: i32,
        had_budget: bool,
        elapsed: Duration,
    ) {
        self.calls += 1;
        self.ns += elapsed.as_nanos();
        if !had_budget || result < -1 {
            return;
        }
        let key = (result == -1, pc, lr, thumb);
        if self.rows.len() >= LIMIT && !self.rows.contains_key(&key) {
            self.overflow_calls += 1;
            self.overflow_ns += elapsed.as_nanos();
            return;
        }
        let count = self.rows.entry(key).or_default();
        count.calls += 1;
        count.ns += elapsed.as_nanos();
    }
    pub fn report(&mut self, elapsed_ms: u128) {
        log!(
            "LEAF_GETTER_WINDOW: elapsed_ms={elapsed_ms} calls={}",
            LEAF_GETTERS.swap(0, Ordering::Relaxed)
        );
        log!(
            "IMAGE_UPLOAD_WINDOW: elapsed_ms={elapsed_ms} fast={} fast_pixels={} fallback={}",
            IMAGE_FAST.swap(0, Ordering::Relaxed),
            IMAGE_FAST_PIXELS.swap(0, Ordering::Relaxed),
            IMAGE_FALLBACK.swap(0, Ordering::Relaxed)
        );
        log!("GUEST_CPU_WINDOW: elapsed_ms={elapsed_ms} calls={} run_us={} keys={} overflow_calls={} overflow_us={}",self.calls,self.ns/1000,self.rows.len(),self.overflow_calls,self.overflow_ns/1000);
        let mut rows: Vec<_> = self.rows.iter().collect();
        rows.sort_unstable_by_key(|(_, count)| std::cmp::Reverse(count.ns));
        for budget in [true, false] {
            for (&(_, pc, lr, thumb), count) in rows
                .iter()
                .copied()
                .filter(|(key, _)| key.0 == budget)
                .take(32)
            {
                log!("GUEST_CPU_TOP: elapsed_ms={elapsed_ms} kind={} pc={pc:#x} lr={lr:#x} thumb={} calls={} run_us={}",if budget {"budget"} else {"svc"},u8::from(thumb),count.calls,count.ns/1000);
            }
        }
        self.rows.clear();
        self.calls = 0;
        self.ns = 0;
        self.overflow_calls = 0;
        self.overflow_ns = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounds_and_return_kinds_preserve_aggregate_coverage() {
        let mut p = Profile::default();
        for pc in 0..LIMIT as u32 + 10 {
            p.record(pc, 0, true, -1, true, Duration::from_nanos(5));
        }
        assert_eq!(p.rows.len(), LIMIT);
        assert_eq!(p.overflow_calls, 10);
        assert_eq!(p.overflow_ns, 50);
        p.record(0, 0, true, -1, true, Duration::from_nanos(7));
        assert_eq!(p.rows[&(true, 0, 0, true)].calls, 2);
        assert_eq!(p.rows[&(true, 0, 0, true)].ns, 12);
        let mut kinds = Profile::default();
        kinds.record(4, 8, false, 123, true, Duration::from_nanos(11));
        kinds.record(4, 8, false, -1, true, Duration::from_nanos(13));
        kinds.record(4, 8, false, -1, false, Duration::from_nanos(17));
        kinds.record(4, 8, false, -2, true, Duration::from_nanos(19));
        assert_eq!(kinds.rows.len(), 2);
        assert_eq!(kinds.ns, 60);
        assert_eq!(kinds.rows[&(false, 4, 8, false)].ns, 11);
        assert_eq!(kinds.rows[&(true, 4, 8, false)].ns, 13);
    }
}
