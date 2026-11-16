//! In-memory token buckets, per IP and per account. Restarting the server resets them (the
//! six-digit code failures that must survive a restart are counted in SQLite, see `codes`).

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

// Per-IP rules are loose: a mobile carrier puts hundreds of phones behind one IPv4 (CGNAT), a
// venue's Wi-Fi a whole party. The per-account rules below stop guessing.
pub const EMAIL_IP: Rule = rule("email_ip", 40, 10);
pub const SIGNUP_IP: Rule = rule("signup_ip", 30, 60);
pub const LOGIN_IP: Rule = rule("login_ip", 30, 10);
/// Wrong passwords for one e-mail from one IP (a success gives its token back); refills one every 3 min.
pub const LOGIN_ACCOUNT_IP: Rule = rule("login_ip_account", 5, 15);
/// Wrong passwords for one e-mail from anywhere: looser, so one IP (at most 20 an hour through the
/// rule above) can't lock the owner out, but guesses spread over many IPs still stop.
pub const LOGIN_ACCOUNT: Rule = rule("login_account", 40, 60);
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
/// Joining a party without an account: a whole party shares one Wi-Fi (three full parties an hour,
/// `party::MAX_PLAYERS` each).
pub const PARTY_GUEST_IP: Rule = rule("party_guest_ip", 120, 60);
/// The public invite preview (`GET /api/rooms/preview`): enough for a few links, too few to scan codes.
pub const ROOM_PREVIEW_IP: Rule = rule("room_preview_ip", 30, 10);
pub const DELETE_SEND_IP: Rule = rule("delete_send_ip", 5, 60);
pub const DELETE_SEND_ACCOUNT: Rule = rule("delete_send_account", 3, 60);
pub const DELETE_CONFIRM_ACCOUNT: Rule = rule("delete_confirm_account", 10, 60);
pub const REPORT_USER: Rule = rule("report_user", 20, 60);
/// Server-wide (one key): beyond this, reports are stored but not emailed.
pub const REPORT_MAIL: Rule = rule("report_mail", 30, 60);
/// App crash reports (`POST /api/client-errors`).
pub const CLIENT_ERROR_IP: Rule = rule("client_error_ip", 20, 10);
pub const CLIENT_ERROR_USER: Rule = rule("client_error_user", 10, 10);
/// Server-wide (one key): beyond this, crash reports are logged but not sent to the webhook.
pub const CLIENT_ERROR_WEBHOOK: Rule = rule("client_error_webhook", 30, 60);

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

pub struct Limiter {
    buckets: Mutex<Buckets>,
    /// Most buckets kept; beyond it the least useful ones are dropped (see `evict`).
    cap: usize,
}

impl Default for Limiter {
    fn default() -> Self {
        Self::with_cap(HARD_CAP)
    }
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

/// Per-IP rules (named `*_ip`) lose their buckets first when memory runs out: an attacker filling
/// the map with fresh IPs must not wipe the per-account failure budgets (sign-in, 2FA, codes).
fn per_ip(name: &str) -> bool {
    name.ends_with("_ip")
}

/// Over the cap even after dropping the full buckets: drop per-IP buckets before per-account ones,
/// the least recently used first, down to 90 % of the cap (so this doesn't run on every call).
fn evict(map: &mut HashMap<(&'static str, String), Bucket>, cap: usize) {
    let keep = cap / 10 * 9;
    if map.len() <= keep {
        return;
    }
    let mut order: Vec<(bool, Instant, (&'static str, String))> = map
        .iter()
        .map(|(k, b)| (!per_ip(k.0), b.at, k.clone()))
        .collect();
    // Per-IP (false) first, then oldest first.
    order.sort_unstable_by_key(|o| (o.0, o.1));
    let drop = map.len() - keep;
    for (_, _, k) in order.into_iter().take(drop) {
        map.remove(&k);
    }
}

impl Limiter {
    pub fn with_cap(cap: usize) -> Self {
        Self {
            buckets: Mutex::default(),
            cap,
        }
    }

    fn with<T>(&self, rule: &Rule, key: &str, f: impl FnOnce(&mut Bucket) -> T) -> T {
        let now = Instant::now();
        let mut guard = self.buckets.lock().unwrap_or_else(|e| e.into_inner());
        let Buckets { map, pruned } = &mut *guard;
        let due = pruned.is_none_or(|t| now.duration_since(t) > Duration::from_secs(10));
        let prune_above = PRUNE_ABOVE.min(self.cap / 2);
        if map.len() >= self.cap || (map.len() > prune_above && due) {
            *pruned = Some(now);
            // Drop the buckets that are full again: they hold no information.
            // (On a copy: `at` stays the last use, which `evict` goes by.)
            map.retain(|(name, _), b| {
                let Some(r) = rule_by_name(name) else {
                    return false;
                };
                let mut b = Bucket {
                    tokens: b.tokens,
                    at: b.at,
                };
                refill(&mut b, &r, now);
                b.tokens < f64::from(r.capacity)
            });
            if map.len() >= self.cap {
                evict(map, self.cap);
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
        LOGIN_ACCOUNT_IP,
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
        PARTY_GUEST_IP,
        ROOM_PREVIEW_IP,
        DELETE_SEND_IP,
        DELETE_SEND_ACCOUNT,
        DELETE_CONFIRM_ACCOUNT,
        REPORT_USER,
        REPORT_MAIL,
        CLIENT_ERROR_IP,
        CLIENT_ERROR_USER,
        CLIENT_ERROR_WEBHOOK,
    ]
    .into_iter()
    .chain(crate::quotas::RULES)
    .find(|r| r.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buckets_empty_then_say_when_to_retry() {
        let l = Limiter::default();
        // Drain the bucket whatever its size, then the wait is about one token's refill time.
        for _ in 0..SIGNUP_IP.capacity {
            l.hit(&SIGNUP_IP, "1.2.3.4").unwrap();
        }
        let one_token = SIGNUP_IP.period.as_secs() / u64::from(SIGNUP_IP.capacity);
        match l.hit(&SIGNUP_IP, "1.2.3.4") {
            Err(AppError::RateLimited(s)) => assert!((1..=one_token).contains(&s), "{s}"),
            other => panic!("{other:?}"),
        }
        l.hit(&SIGNUP_IP, "5.6.7.8").unwrap();
        // Successes give their token back, failures don't.
        for _ in 0..10 {
            l.hit(&LOGIN_ACCOUNT_IP, "a@b.c").unwrap();
            l.refund(&LOGIN_ACCOUNT_IP, "a@b.c");
        }
        for _ in 0..LOGIN_ACCOUNT_IP.capacity {
            l.hit(&LOGIN_ACCOUNT_IP, "a@b.c").unwrap();
        }
        assert!(l.hit(&LOGIN_ACCOUNT_IP, "a@b.c").is_err());
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

    #[test]
    fn a_flood_of_ips_does_not_wipe_account_budgets() {
        let l = Limiter::with_cap(1_000);
        // The victim's account has used up its sign-in failures.
        for _ in 0..5 {
            l.hit(&LOGIN_ACCOUNT_IP, "ana@example.com|1.2.3.4").unwrap();
        }
        assert!(l.hit(&LOGIN_ACCOUNT_IP, "ana@example.com|1.2.3.4").is_err());
        // An attacker fills the map with half-used per-IP buckets from many addresses.
        for i in 0..5_000 {
            l.hit(&LOGIN_IP, &format!("10.0.{}.{}", i / 256, i % 256))
                .unwrap();
        }
        let len = l.buckets.lock().unwrap().map.len();
        assert!(len <= 1_000, "{len}");
        // The account budget is still spent; the newest IP buckets are still there.
        assert!(l.hit(&LOGIN_ACCOUNT_IP, "ana@example.com|1.2.3.4").is_err());
        let map = &l.buckets.lock().unwrap().map;
        assert!(map.contains_key(&("login_ip", "10.0.19.135".to_owned())));
        assert!(!map.contains_key(&("login_ip", "10.0.0.0".to_owned())));
    }
}
