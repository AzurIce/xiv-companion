use std::collections::{HashMap, HashSet};

// Scheduler, registration order, shared Timeline versus flat root, storage slot.
pub(super) type PrepareEntry = (usize, usize, bool, usize);
type Key = (bool, usize);

/// Scheduler-owned siblings. Each callback returns before the caller reads
/// the current storage slot's next link (native Scheduler +70, 3bc3de).
/// Detached slots retain their old link until a constructor/attachment reuses
/// them; reattachment resets that link, even for the currently visited slot.
pub(super) struct PrepareLinks {
    grouped: bool,
    groups: Vec<usize>,
    heads: HashMap<usize, Option<Key>>,
    tails: HashMap<usize, Option<Key>>,
    next: HashMap<Key, Option<Key>>,
    entries: HashMap<Key, PrepareEntry>,
    registered: HashSet<Key>,
}

impl PrepareLinks {
    pub fn new(entries: &[PrepareEntry], grouped: bool) -> Self {
        let mut links = Self {
            grouped,
            groups: Vec::new(),
            heads: HashMap::new(),
            tails: HashMap::new(),
            next: HashMap::new(),
            entries: HashMap::new(),
            registered: HashSet::new(),
        };
        for &entry in entries {
            links.append(entry);
        }
        links
    }

    fn group(&self, entry: PrepareEntry) -> usize {
        if self.grouped { entry.0 } else { 0 }
    }

    pub fn append(&mut self, entry: PrepareEntry) {
        let key = (entry.2, entry.3);
        assert!(
            self.registered.insert(key),
            "duplicate Scheduler attachment"
        );
        let group = self.group(entry);
        self.ensure_group(group);
        self.next.insert(key, None);
        self.entries.insert(key, entry);
        if let Some(tail) = self.tails[&group] {
            self.next.insert(tail, Some(key));
        } else {
            self.heads.insert(group, Some(key));
        }
        self.tails.insert(group, Some(key));
    }

    /// A Scheduler can own pending factories without any registered child.
    pub fn ensure_group(&mut self, group: usize) {
        if !self.heads.contains_key(&group) {
            self.groups.push(group);
            self.heads.insert(group, None);
            self.tails.insert(group, None);
        }
    }

    pub fn groups(&self) -> Vec<usize> {
        self.groups.clone()
    }

    pub fn first_in_group(&self, group: usize) -> Option<PrepareEntry> {
        self.heads[&group].map(|key| self.entries[&key])
    }

    pub fn after_in_group(&self, entry: PrepareEntry) -> Option<PrepareEntry> {
        self.next[&(entry.2, entry.3)].map(|key| self.entries[&key])
    }

    pub fn detach(&mut self, entry: PrepareEntry) {
        let key = (entry.2, entry.3);
        assert!(self.registered.remove(&key), "missing Scheduler attachment");
        let group = self.group(entry);
        let mut previous = None;
        let mut current = self.heads[&group];
        while let Some(slot) = current {
            if slot == key {
                let next = self.next[&slot];
                if let Some(previous) = previous {
                    self.next.insert(previous, next);
                } else {
                    self.heads.insert(group, next);
                }
                if self.tails[&group] == Some(slot) {
                    self.tails.insert(group, previous);
                }
                return;
            }
            previous = current;
            current = self.next[&slot];
        }
        unreachable!("registered Scheduler slot must be linked")
    }

    #[cfg(test)]
    pub fn first(&self) -> Option<PrepareEntry> {
        self.groups
            .iter()
            .find_map(|group| self.heads[group].map(|key| self.entries[&key]))
    }

    #[cfg(test)]
    pub fn after(&self, entry: PrepareEntry) -> Option<PrepareEntry> {
        let key = (entry.2, entry.3);
        if let Some(next) = self.next[&key] {
            return Some(self.entries[&next]);
        }
        // A self-replacement may end this Scheduler's walk, but Document
        // traversal can still enter the next independent Scheduler group.
        let group = self.group(entry);
        let position = self.groups.iter().position(|g| *g == group).unwrap();
        self.groups[position + 1..]
            .iter()
            .find_map(|g| self.heads[g].map(|key| self.entries[&key]))
    }

    /// Numeric starts a new traversal at each Scheduler's current head.
    pub fn entries(&self) -> Vec<PrepareEntry> {
        let mut entries = Vec::with_capacity(self.registered.len());
        for group in &self.groups {
            let mut current = self.heads[group];
            while let Some(slot) = current {
                entries.push(self.entries[&slot]);
                current = self.next[&slot];
            }
        }
        entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_replacement_skips_later_siblings_but_next_phase_starts_at_new_head() {
        let entries = [
            (0, 0, true, 0),
            (0, 1, true, 1),
            (0, 2, true, 2),
            (1, 3, true, 3),
        ];
        let mut links = PrepareLinks::new(&entries, true);
        let current = links.first().unwrap();
        links.detach(current);
        links.append((0, 4, true, 0));
        assert_eq!(links.after(current), Some(entries[3]));
        assert_eq!(
            links.entries(),
            vec![entries[1], entries[2], (0, 4, true, 0), entries[3]]
        );
    }

    #[test]
    #[ignore = "CPU: requires scripts/probe-item-rest-walk.py exact-client Scheduler observations"]
    fn compare_original_scheduler_rest_live_links() {
        use serde_json::json;
        let directory =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/weapon-vfx-audit");
        let original: serde_json::Value = serde_json::from_slice(
            &std::fs::read(directory.join("item-rest-walk-client-probe.json")).unwrap(),
        )
        .unwrap();
        let mut visits = 0;
        for (ordinal, case) in original["cases"].as_array().unwrap().iter().enumerate() {
            let entries = case["initial"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
                .map(|(order, node)| (0, order, true, node["slot"].as_u64().unwrap() as usize))
                .collect::<Vec<_>>();
            let mut links = PrepareLinks::new(&entries, true);
            let target = case["target"].as_i64().unwrap();
            let mut observed = Vec::new();
            let mut current = links.first();
            let mut order = entries.len();
            while let Some(entry) = current {
                observed.push(entry.3);
                if entry.3 == 0 && (0..3).contains(&target) {
                    for _ in 0..case["repeats"].as_u64().unwrap() {
                        let old = links
                            .entries()
                            .into_iter()
                            .find(|e| e.3 == target as usize)
                            .unwrap();
                        links.detach(old);
                        links.append((0, order, true, target as usize));
                        order += 1;
                    }
                }
                current = links.after(entry);
            }
            let expected = case["events"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|event| event["kind"] == "prepare")
                .map(|event| event["node"]["slot"].as_u64().unwrap() as usize)
                .collect::<Vec<_>>();
            assert_eq!(observed, expected, "case {ordinal} visits");
            let final_slots = links.entries().iter().map(|e| e.3).collect::<Vec<_>>();
            let expected_slots = case["final"]
                .as_array()
                .unwrap()
                .iter()
                .map(|node| node["slot"].as_u64().unwrap() as usize)
                .collect::<Vec<_>>();
            assert_eq!(
                final_slots, expected_slots,
                "case {ordinal} final registration"
            );
            visits += observed.len();
        }
        std::fs::write(directory.join("item-rest-links-rust-comparison.json"),
            serde_json::to_string_pretty(&json!({"cases":270,"prepareVisits":visits,"differences":0,
                "productionLiveLinksCompared":true,"productionRestFactoryCompared":false,
                "scope":"Actual Scheduler link component; controlled REST callbacks detach/reattach selected slots. No full owned playback, factory, destructor, Point/Binder, retired Scheduler, host or GPU."})).unwrap()).unwrap();
    }
}
