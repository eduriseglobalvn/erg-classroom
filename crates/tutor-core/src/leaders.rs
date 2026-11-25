//! Group leaders: a student the tutor trusts for a while to message, lock or unlock their own group.
//! The tutor stays the only machine that sends commands; a leader's request arrives as an event on
//! that student's authenticated connection and is checked here before anything is forwarded.
use erg_proto::{Command, LeaderAction};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Default)]
pub struct Leaders {
    grants: HashMap<Uuid, (HashSet<Uuid>, Instant)>,
}

impl Leaders {
    /// Allow `leader` to act on `members` (never on itself or other leaders' groups) for `ttl`.
    pub fn grant(&mut self, leader: Uuid, members: &[Uuid], ttl: Duration, now: Instant) {
        let set: HashSet<Uuid> = members.iter().copied().filter(|m| *m != leader).collect();
        self.grants.insert(leader, (set, now + ttl));
    }

    pub fn revoke(&mut self, leader: Uuid) {
        self.grants.remove(&leader);
    }

    /// Targets and the command to send, or None when `from` is no leader, the grant ran out, or the request is empty.
    pub fn authorize(&mut self, from: Uuid, action: &LeaderAction, now: Instant) -> Option<(Vec<Uuid>, Command)> {
        let (members, until) = self.grants.get(&from)?;
        if now >= *until {
            self.grants.remove(&from);
            return None;
        }
        let cmd = match action {
            LeaderAction::Message(t) if !t.trim().is_empty() => Command::Message { text: t.chars().take(500).collect(), seconds: 15 },
            LeaderAction::Message(_) => return None,
            LeaderAction::Lock => Command::Lock { blank: false, text: Some("Trưởng nhóm đã khoá máy".into()) },
            LeaderAction::Unlock => Command::Unlock,
        };
        Some((members.iter().copied().collect(), cmd))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_granted_leader_acts_and_only_on_its_members_until_the_grant_expires() {
        let (l, a, b, other) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        let t0 = Instant::now();
        let mut g = Leaders::default();
        assert!(g.authorize(l, &LeaderAction::Lock, t0).is_none(), "not a leader yet");
        g.grant(l, &[a, b, l], Duration::from_secs(60), t0);
        let (targets, cmd) = g.authorize(l, &LeaderAction::Lock, t0).unwrap();
        assert_eq!(targets.iter().copied().collect::<HashSet<_>>(), HashSet::from([a, b]), "not itself");
        assert!(matches!(cmd, Command::Lock { .. }));
        assert!(g.authorize(other, &LeaderAction::Unlock, t0).is_none(), "another student cannot borrow the grant");
        assert!(g.authorize(l, &LeaderAction::Message("  ".into()), t0).is_none());
        assert!(g.authorize(l, &LeaderAction::Unlock, t0 + Duration::from_secs(61)).is_none(), "expired");
        assert!(g.authorize(l, &LeaderAction::Unlock, t0).is_none(), "expiry removes the grant");
        g.grant(l, &[a], Duration::from_secs(60), t0);
        g.revoke(l);
        assert!(g.authorize(l, &LeaderAction::Unlock, t0).is_none());
    }
}
