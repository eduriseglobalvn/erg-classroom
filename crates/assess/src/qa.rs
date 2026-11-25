//! Oral Q&A helpers, survey tally and reward stars. Students are identified by any string key.
use std::collections::BTreeMap;

/// Survey results. A student answering twice replaces the earlier answer.
pub struct Tally {
    options: usize,
    votes: BTreeMap<String, u8>,
}

impl Tally {
    pub fn new(options: usize) -> Self {
        Self { options, votes: BTreeMap::new() }
    }
    /// False when `choice` is not one of the options (answer ignored).
    pub fn vote(&mut self, who: &str, choice: u8) -> bool {
        if (choice as usize) >= self.options {
            return false;
        }
        self.votes.insert(who.to_string(), choice);
        true
    }
    pub fn counts(&self) -> Vec<usize> {
        let mut c = vec![0; self.options];
        for &v in self.votes.values() {
            c[v as usize] += 1;
        }
        c
    }
    pub fn answered(&self) -> usize {
        self.votes.len()
    }
    /// Index of the most voted option; ties give the lowest index; None with no votes.
    pub fn winner(&self) -> Option<usize> {
        let c = self.counts();
        let max = *c.iter().max()?;
        (max > 0).then(|| c.iter().position(|&x| x == max).unwrap())
    }
}

/// Pick one entry from `pool`, never one in `already` unless everybody has been picked.
pub fn pick_random<'a>(pool: &'a [String], already: &[String], seed: u64) -> Option<&'a String> {
    let fresh: Vec<&String> = pool.iter().filter(|p| !already.contains(p)).collect();
    let src: Vec<&String> = if fresh.is_empty() { pool.iter().collect() } else { fresh };
    if src.is_empty() {
        return None;
    }
    let mut s = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x2545_F491_4F6C_DD1D;
    s ^= s << 13;
    s ^= s >> 7;
    s ^= s << 17;
    Some(src[(s % src.len() as u64) as usize])
}

/// Who raised a hand first: (student, timestamp ms). Earliest wins; equal times go to the first listed.
pub fn first_hand(hands: &[(String, u64)]) -> Option<&str> {
    hands.iter().min_by_key(|h| h.1).map(|h| h.0.as_str())
}

/// Team points. A single change is capped so a typo cannot ruin the table.
#[derive(Default)]
pub struct Scoreboard(BTreeMap<String, i32>);

impl Scoreboard {
    pub fn add(&mut self, team: &str, delta: i32) -> i32 {
        let e = self.0.entry(team.to_string()).or_insert(0);
        *e = e.saturating_add(delta.clamp(-100, 100));
        *e
    }
    /// Highest first; equal scores alphabetical.
    pub fn ranking(&self) -> Vec<(String, i32)> {
        let mut v: Vec<_> = self.0.iter().map(|(k, v)| (k.clone(), *v)).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        v
    }
}

/// Reward stars per student, capped so a typo cannot show 65 000 stars on a toolbar.
#[derive(Default)]
pub struct Rewards(BTreeMap<String, u16>);

impl Rewards {
    pub const MAX: u16 = 99;
    pub fn give(&mut self, who: &str, n: u16) -> u16 {
        let e = self.0.entry(who.to_string()).or_insert(0);
        *e = e.saturating_add(n).min(Self::MAX);
        *e
    }
    pub fn get(&self, who: &str) -> u16 {
        self.0.get(who).copied().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn tally_replaces_repeat_votes_and_ignores_bad_choices() {
        let mut t = Tally::new(3);
        assert!(t.vote("a", 0) && t.vote("b", 1) && t.vote("a", 1));
        assert!(!t.vote("c", 3));
        assert_eq!((t.counts(), t.answered(), t.winner()), (vec![0, 2, 0], 2, Some(1)));
        assert_eq!(Tally::new(2).winner(), None);
        let mut tie = Tally::new(2);
        tie.vote("a", 1);
        tie.vote("b", 0);
        assert_eq!(tie.winner(), Some(0));
    }

    #[test]
    fn random_pick_skips_picked_until_all_were_picked() {
        let pool = names(&["a", "b", "c"]);
        for seed in 0..50 {
            assert_ne!(pick_random(&pool, &names(&["a", "b"]), seed).unwrap(), "a");
        }
        assert!(pick_random(&pool, &pool, 1).is_some());
        assert!(pick_random(&[], &[], 1).is_none());
        let seen: std::collections::HashSet<_> = (0..200).filter_map(|s| pick_random(&pool, &[], s)).collect();
        assert_eq!(seen.len(), 3, "all students must be reachable");
    }

    #[test]
    fn first_hand_and_scoreboard() {
        assert_eq!(first_hand(&[("b".into(), 20), ("a".into(), 10), ("c".into(), 10)]), Some("a"));
        assert_eq!(first_hand(&[]), None);
        let mut s = Scoreboard::default();
        s.add("red", 5);
        s.add("blue", 5);
        s.add("blue", 1000);
        s.add("red", -2);
        assert_eq!(s.ranking(), vec![("blue".to_string(), 105), ("red".to_string(), 3)]);
    }

    #[test]
    fn rewards_are_capped() {
        let mut r = Rewards::default();
        assert_eq!(r.give("a", 60), 60);
        assert_eq!(r.give("a", 60), Rewards::MAX);
        assert_eq!(r.give("a", u16::MAX), Rewards::MAX);
        assert_eq!(r.get("zzz"), 0);
    }
}
