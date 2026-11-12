//! In-memory token buckets, per IP and per account. Restarting the server resets them (fine).

use std::{
    collections::HashMap,
    net::IpAddr,
    sync::Mutex,
    time::{Duration, Instant},
};

use crate::error::AppError;

#[derive(Clone, Copy, Debug)]
pub struct Rule {
    pub name: &'static str,
    pub capacity: u32,
    /// Time to refill the whole bucket.
    pub period: Duration,
}

const fn rule(name: &'static str, capacity: u32, minutes: u64) -> Rule {
    Rule {
        name,
        capacity,
        period: Duration::from_secs(minutes * 60),
    }
}

pub const EMAIL_IP: Rule = rule("email_ip", 20, 10);
pub const SIGNUP_IP: Rule = rule("signup_ip", 5, 60);
pub const LOGIN_IP: Rule = rule("login_ip", 10, 10);
/// Failures only (a success gives its token back); refills one every 3 min.
pub const LOGIN_ACCOUNT: Rule = rule("login_account", 5, 15);
pub const MFA_IP: Rule = rule("mfa_ip", 10, 10);
/// Failures only (a success gives its token back).
pub const MFA_ACCOUNT: Rule = rule("mfa_account", 5, 15);
pub const FORGOT_IP: Rule = rule("forgot_ip", 5, 60);
pub const FORGOT_ACCOUNT: Rule = rule("forgot_account", 3, 60);
pub const CODE_IP: Rule = rule("code_ip", 10, 10);
pub const RESET_ACCOUNT: Rule = rule("reset_account", 10, 60);
pub const VERIFY_SEND_ACCOUNT: Rule = rule("verify_send_account", 5, 60);
pub const OAUTH_IP: Rule = rule("oauth_ip", 20, 10);
pub const LEGACY_IP: Rule = rule("legacy_ip", 10, 60);

struct Bucket {
    tokens: f64,
    at: Instant,
}

const PRUNE_ABOVE: usize = 10_000;
const HARD_CAP: usize = 100_000;

#[derive(Default)]
struct Buckets {
    map: HashMap<(&'static str, String), Bucket>,
    pruned: Option<Instant>,
}

#[derive(Default)]
pub struct Limiter {
    buckets: Mutex<Buckets>,
}

fn refill(b: &mut Bucket, rule: &Rule, now: Instant) {
    let rate = f64::from(rule.capacity) / rule.period.as_secs_f64();
    b.tokens =
        (b.tokens + now.duration_since(b.at).as_secs_f64() * rate).min(f64::from(rule.capacity));
    b.at = now;
}

fn retry_after(b: &Bucket, rule: &Rule) -> u64 {
    let rate = f64::from(rule.capacity) / rule.period.as_secs_f64();
    ((1.0 - b.tokens) / rate).ceil().max(1.0) as u64
}

pub fn ip_key(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(v4) => v4.to_string(),
        // One bucket per /64: a single host usually owns a whole /64.
        IpAddr::V6(v6) => {
            let s = v6.segments();
            format!("{:x}:{:x}:{:x}:{:x}::/64", s[0], s[1], s[2], s[3])
        }
    }
}

impl Limiter {
    fn with<T>(&self, rule: &Rule, key: &str, f: impl FnOnce(&mut Bucket) -> T) -> T {
        let now = Instant::now();
        let mut guard = self.buckets.lock().unwrap_or_else(|e| e.into_inner());
        let Buckets { map, pruned } = &mut *guard;
        let due = pruned.is_none_or(|t| now.duration_since(t) > Duration::from_secs(10));
        if map.len() > HARD_CAP || (map.len() > PRUNE_ABOVE && due) {
            *pruned = Some(now);
            // Drop the buckets that are full again: they hold no information.
            map.retain(|(name, _), b| {
                let Some(r) = rule_by_name(name) else {
                    return false;
                };
                refill(b, &r, now);
                b.tokens < f64::from(r.capacity)
            });
            if map.len() > HARD_CAP {
                map.clear();
            }
        }
        let b = map.entry((rule.name, key.to_owned())).or_insert(Bucket {
            tokens: f64::from(rule.capacity),
            at: now,
        });
        refill(b, rule, now);
        f(b)
    }

    /// Takes one token, or says how long to wait.
    pub fn hit(&self, rule: &Rule, key: &str) -> Result<(), AppError> {
        self.with(rule, key, |b| {
            if b.tokens >= 1.0 {
                b.tokens -= 1.0;
                Ok(())
            } else {
                Err(AppError::RateLimited(retry_after(b, rule)))
            }
        })
    }

    /// Per-IP rule; nothing to count when the IP is unknown (no socket info, e.g. in-process tests).
    pub fn hit_ip(&self, rule: &Rule, ip: Option<IpAddr>) -> Result<(), AppError> {
        match ip {
            Some(ip) => self.hit(rule, &ip_key(ip)),
            None => Ok(()),
        }
    }

    /// Failure-counting rules: every attempt takes a token up front (with `hit`, so parallel
    /// attempts can't all slip through), a success gives it back.
    pub fn refund(&self, rule: &Rule, key: &str) {
        self.with(rule, key, |b| {
            b.tokens = (b.tokens + 1.0).min(f64::from(rule.capacity));
        });
    }
}

fn rule_by_name(name: &str) -> Option<Rule> {
    [
        EMAIL_IP,
        SIGNUP_IP,
        LOGIN_IP,
        LOGIN_ACCOUNT,
        MFA_IP,
        MFA_ACCOUNT,
        FORGOT_IP,
        FORGOT_ACCOUNT,
        CODE_IP,
        RESET_ACCOUNT,
        VERIFY_SEND_ACCOUNT,
        OAUTH_IP,
        LEGACY_IP,
    ]
    .into_iter()
    .find(|r| r.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buckets_empty_then_say_when_to_retry() {
        let l = Limiter::default();
        for _ in 0..5 {
            l.hit(&SIGNUP_IP, "1.2.3.4").unwrap();
        }
        match l.hit(&SIGNUP_IP, "1.2.3.4") {
            Err(AppError::RateLimited(s)) => assert!((1..=720).contains(&s), "{s}"),
            other => panic!("{other:?}"),
        }
        l.hit(&SIGNUP_IP, "5.6.7.8").unwrap();
        // Successes give their token back, failures don't.
        for _ in 0..10 {
            l.hit(&LOGIN_ACCOUNT, "a@b.c").unwrap();
            l.refund(&LOGIN_ACCOUNT, "a@b.c");
        }
        for _ in 0..5 {
            l.hit(&LOGIN_ACCOUNT, "a@b.c").unwrap();
        }
        assert!(l.hit(&LOGIN_ACCOUNT, "a@b.c").is_err());
    }

    #[test]
    fn memory_stays_bounded() {
        let l = Limiter::default();
        for i in 0..(PRUNE_ABOVE + 10) {
            l.hit(&EMAIL_IP, &i.to_string()).unwrap();
        }
        l.hit(&EMAIL_IP, "last").unwrap();
        // Nothing is full again yet, so the map is still large but capped.
        assert!(l.buckets.lock().unwrap().map.len() <= HARD_CAP);
    }
}
