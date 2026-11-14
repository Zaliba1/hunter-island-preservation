/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Opt-in legacy cleanup for known texture names deleted without a context.
//! Ambiguous/unknown names keep the ordinary no-context behavior.
use std::collections::HashMap;
pub(super) const LIMIT: usize = 16_384;
#[derive(Default)]
pub(super) struct Cleanup {
    names: HashMap<(usize, u32), bool>,
    pending: usize,
    pub deferred: u64,
    pub flushed: u64,
}
impl Cleanup {
    pub fn remember(&mut self, group: usize, names: &[u32]) {
        for &name in names {
            if name != 0 && self.names.len() < LIMIT {
                self.names.entry((group, name)).or_insert(false);
            }
        }
    }
    pub fn delete(&mut self, group: usize, names: &[u32]) {
        for &name in names {
            if self.names.remove(&(group, name)) == Some(true) {
                self.pending -= 1;
            }
        }
    }
    pub fn defer(&mut self, names: &[u32]) -> usize {
        let mut accepted = 0;
        for &name in names {
            if name == 0 {
                continue;
            }
            let mut matches = self.names.keys().filter(|(_, n)| *n == name);
            let Some(&key) = matches.next() else {
                continue;
            };
            if matches.next().is_some() {
                continue;
            }
            let pending = self.names.get_mut(&key).unwrap();
            if !*pending {
                *pending = true;
                self.pending += 1;
                self.deferred += 1;
            }
            accepted += 1;
        }
        accepted
    }
    pub fn has_pending(&self) -> bool {
        self.pending != 0
    }
    pub fn take(&mut self, group: usize) -> Vec<u32> {
        let mut result = Vec::new();
        self.names.retain(|&(owner, name), pending| {
            if owner == group && *pending {
                result.push(name);
                false
            } else {
                true
            }
        });
        self.pending -= result.len();
        self.flushed += result.len() as u64;
        result
    }
    pub fn forget_group(&mut self, group: usize) {
        let mut removed = 0;
        self.names.retain(|&(owner, _), pending| {
            if owner == group {
                removed += usize::from(*pending);
                false
            } else {
                true
            }
        });
        self.pending -= removed;
    }
    #[cfg(feature = "diagnostic-live-cycles")]
    pub fn counts(&self) -> (usize, usize, u64, u64) {
        (self.names.len(), self.pending, self.deferred, self.flushed)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deletes_wait_for_the_unique_owner_and_never_cross_sharegroups() {
        let mut c = Cleanup::default();
        c.remember(1, &[10, 11]);
        c.remember(2, &[10, 12]);
        assert_eq!(c.defer(&[0, 999, 10]), 0);
        assert_eq!(c.defer(&[11, 11]), 2);
        assert_eq!(c.pending, 1);
        assert!(c.take(2).is_empty());
        assert_eq!(c.take(1), [11]);
        c.delete(2, &[10]);
        assert_eq!(c.defer(&[10]), 1);
        assert_eq!(c.take(1), [10]);
        c.remember(1, &[10]);
        assert!(!c.has_pending()); // a reused native name is a new live object
        assert_eq!(c.defer(&[12]), 1);
        c.forget_group(2);
        assert!(!c.has_pending());
    }
    #[test]
    fn tracking_and_pending_storage_stay_bounded() {
        let mut c = Cleanup::default();
        let names: Vec<_> = (1..=LIMIT as u32 + 100).collect();
        c.remember(1, &names);
        assert_eq!(c.names.len(), LIMIT);
        assert_eq!(c.defer(&names), LIMIT);
        assert_eq!(c.pending, LIMIT);
        c.forget_group(1);
        assert!(c.names.is_empty());
        assert!(!c.has_pending());
    }
}
