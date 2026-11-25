//! What the SYSTEM service does with a request that passed `erg_privcmd::Gate`. Holds the long-running
//! blockers (app policy, unmanaged-browser guard) so they outlive any user session.
use crate::apps::{browser_guard, AppBlocker, Policy};
use erg_proto::{AppPolicy, PowerAction, PrivAction, WebPolicy};
use std::sync::Mutex;

pub struct Runner {
    apps: Mutex<Option<AppBlocker>>,
    guard: Mutex<Option<AppBlocker>>,
    /// (session id, SID) of each user signed in right now: Protect is per-user, Logoff is per-session.
    users: Box<dyn Fn() -> Vec<(u32, String)> + Send + Sync>,
    /// `allow_power=1` in the machine config: Shutdown/Reboot/Logoff from the tutor are only honoured then.
    allow_power: bool,
}

impl Runner {
    pub fn new(users: impl Fn() -> Vec<(u32, String)> + Send + Sync + 'static, allow_power: bool) -> Self {
        Self { apps: Default::default(), guard: Default::default(), users: Box::new(users), allow_power }
    }

    pub fn run(&self, a: PrivAction) -> Result<(), String> {
        match a {
            PrivAction::Web(mode, list) => {
                crate::web::apply(mode, &list)?;
                let mut g = self.guard.lock().expect("guard lock");
                if mode == WebPolicy::Unrestricted {
                    *g = None;
                } else if g.is_none() {
                    *g = Some(AppBlocker::start_for_users(browser_guard(), |_| {}));
                }
                Ok(())
            }
            PrivAction::App(mode, list) => {
                let p = Policy::new(mode, list).map_err(str::to_string)?;
                let mut g = self.apps.lock().expect("apps lock");
                match g.as_ref() {
                    Some(b) => b.set_policy(p),
                    None if mode != AppPolicy::Unrestricted => *g = Some(AppBlocker::start_for_users(p, |_| {})),
                    None => {}
                }
                Ok(())
            }
            PrivAction::Usb(p) => crate::usb::apply(p).map_err(|e| e.to_string()),
            PrivAction::Print(p) => {
                let failed = crate::print::apply(p);
                if failed.is_empty() { Ok(()) } else { Err(format!("not applied to {failed:?}")) }
            }
            PrivAction::Protect(p) => {
                let errs: Vec<String> = (self.users)().iter().filter_map(|(_, s)| crate::protect::apply_for_sid(s, &p).err()).collect();
                if errs.is_empty() { Ok(()) } else { Err(errs.join("; ")) }
            }
            PrivAction::Power(p) => {
                let sessions: Vec<u32> = (self.users)().iter().map(|(s, _)| *s).collect();
                let cmds = power_commands(p, self.allow_power, &sessions)?;
                for (prog, args) in cmds {
                    std::process::Command::new(prog).args(args).spawn().map_err(|e| e.to_string())?;
                }
                Ok(())
            }
        }
    }
}

/// Programs to run for a power request. Off unless the machine config allows it. Logoff ends each signed-in
/// session (`logoff <id>`): `shutdown /l` from the service would only target the service's own session.
pub fn power_commands(p: PowerAction, allowed: bool, sessions: &[u32]) -> Result<Vec<(&'static str, Vec<String>)>, String> {
    if !allowed {
        return Err("power commands are disabled on this machine".into());
    }
    let s = |a: &[&str]| a.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    Ok(match p {
        PowerAction::Shutdown => vec![("shutdown.exe", s(&["/s", "/t", "10"]))],
        PowerAction::Reboot => vec![("shutdown.exe", s(&["/r", "/t", "10"]))],
        PowerAction::Logoff => sessions.iter().map(|id| ("logoff.exe", vec![id.to_string()])).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn power_needs_the_machine_flag_and_logoff_targets_the_signed_in_sessions() {
        assert!(power_commands(PowerAction::Shutdown, false, &[1]).is_err());
        assert_eq!(power_commands(PowerAction::Reboot, true, &[]).unwrap()[0].1[0], "/r");
        let l = power_commands(PowerAction::Logoff, true, &[2, 5]).unwrap();
        assert_eq!(l, vec![("logoff.exe", vec!["2".to_string()]), ("logoff.exe", vec!["5".to_string()])]);
    }

    #[test]
    fn app_policy_goes_through_the_same_checks_and_an_empty_allow_list_is_refused() {
        let r = Runner::new(Vec::new, false);
        assert!(r.run(PrivAction::App(AppPolicy::AllowListOnly, vec![])).is_err());
        assert!(r.run(PrivAction::App(AppPolicy::Unrestricted, vec![])).is_ok());
        assert!(r.apps.lock().unwrap().is_none(), "unrestricted does not start a watcher");
        assert!(r.run(PrivAction::Power(PowerAction::Shutdown)).is_err(), "power disabled by default");
    }

    #[test]
    fn protect_without_signed_in_users_is_a_no_op() {
        let r = Runner::new(Vec::new, false);
        assert!(r.run(PrivAction::Protect(Default::default())).is_ok());
    }
}
