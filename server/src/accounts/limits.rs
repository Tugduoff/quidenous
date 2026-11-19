use std::{
    collections::HashMap,
    net::IpAddr,
    sync::Mutex,
    time::{Duration, Instant},
};

use crate::platform::error::AppError;

#[derive(Clone, Copy, Debug)]
pub struct Rule {
    pub name: &'static str,
    pub capacity: u32,
    pub period: Duration,
}

const fn rule(name: &'static str, capacity: u32, minutes: u64) -> Rule {
    Rule {
        name,
        capacity,
        period: Duration::from_secs(minutes * 60),
    }
}

pub(crate) const EMAIL_IP: Rule = rule("email_ip", 40, 10);
pub const SIGNUP_IP: Rule = rule("signup_ip", 30, 60);
pub const LOGIN_IP: Rule = rule("login_ip", 30, 10);
pub(crate) const LOGIN_ACCOUNT_IP: Rule = rule("login_ip_account", 5, 15);
pub(crate) const LOGIN_ACCOUNT: Rule = rule("login_account", 40, 60);
pub(crate) const MFA_IP: Rule = rule("mfa_ip", 10, 10);
pub(crate) const MFA_ACCOUNT: Rule = rule("mfa_account", 5, 15);
pub(crate) const FORGOT_IP: Rule = rule("forgot_ip", 5, 60);
pub(crate) const FORGOT_ACCOUNT: Rule = rule("forgot_account", 3, 60);
pub(crate) const CODE_IP: Rule = rule("code_ip", 10, 10);
pub(crate) const RESET_ACCOUNT: Rule = rule("reset_account", 10, 60);
pub(crate) const VERIFY_SEND_ACCOUNT: Rule = rule("verify_send_account", 5, 60);
pub(crate) const OAUTH_IP: Rule = rule("oauth_ip", 20, 10);
pub(crate) const LEGACY_IP: Rule = rule("legacy_ip", 10, 60);
pub const PARTY_GUEST_IP: Rule = rule("party_guest_ip", 120, 60);
pub(crate) const ROOM_PREVIEW_IP: Rule = rule("room_preview_ip", 30, 10);
pub(crate) const DELETE_SEND_IP: Rule = rule("delete_send_ip", 5, 60);
pub(crate) const DELETE_SEND_ACCOUNT: Rule = rule("delete_send_account", 3, 60);
pub(crate) const DELETE_CONFIRM_ACCOUNT: Rule = rule("delete_confirm_account", 10, 60);
pub(crate) const REPORT_USER: Rule = rule("report_user", 20, 60);
pub(crate) const REPORT_MAIL: Rule = rule("report_mail", 30, 60);
pub(crate) const CLIENT_ERROR_IP: Rule = rule("client_error_ip", 20, 10);
pub(crate) const CLIENT_ERROR_USER: Rule = rule("client_error_user", 10, 10);
pub const CLIENT_ERROR_WEBHOOK_IP: Rule = rule("client_error_webhook_ip", 3, 60);
pub(crate) const CLIENT_ERROR_WEBHOOK: Rule = rule("client_error_webhook", 30, 60);

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

pub(crate) fn ip_key(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(v4) => v4.to_string(),
        IpAddr::V6(v6) => {
            let s = v6.segments();
            format!("{:x}:{:x}:{:x}:{:x}::/64", s[0], s[1], s[2], s[3])
        }
    }
}

fn per_ip(name: &str) -> bool {
    name.ends_with("_ip")
}

fn evict(map: &mut HashMap<(&'static str, String), Bucket>, cap: usize) {
    let keep = cap / 10 * 9;
    if map.len() <= keep {
        return;
    }
    let mut order: Vec<(bool, Instant, (&'static str, String))> = map
        .iter()
        .map(|(k, b)| (!per_ip(k.0), b.at, k.clone()))
        .collect();
    order.sort_unstable_by_key(|o| (o.0, o.1));
    let drop = map.len() - keep;
    for (_, _, k) in order.into_iter().take(drop) {
        map.remove(&k);
    }
}

impl Limiter {
    pub(crate) fn with_cap(cap: usize) -> Self {
        Self {
            buckets: Mutex::default(),
            cap,
        }
    }

    fn with<T>(&self, rule: &Rule, key: &str, f: impl FnOnce(&mut Bucket) -> T) -> T {
        let now = Instant::now();
        let mut guard = self
            .buckets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Buckets { map, pruned } = &mut *guard;
        let due = pruned.is_none_or(|t| now.duration_since(t) > Duration::from_secs(10));
        let prune_above = PRUNE_ABOVE.min(self.cap / 2);
        if map.len() >= self.cap || (map.len() > prune_above && due) {
            *pruned = Some(now);
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

    pub(crate) fn hit_ip(&self, rule: &Rule, ip: Option<IpAddr>) -> Result<(), AppError> {
        match ip {
            Some(ip) => self.hit(rule, &ip_key(ip)),
            None => Ok(()),
        }
    }

    pub(crate) fn refund(&self, rule: &Rule, key: &str) {
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
        CLIENT_ERROR_WEBHOOK_IP,
        CLIENT_ERROR_WEBHOOK,
    ]
    .into_iter()
    .chain(crate::platform::quotas::RULES)
    .find(|r| r.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buckets_empty_then_say_when_to_retry() {
        let l = Limiter::default();
        for _ in 0..SIGNUP_IP.capacity {
            l.hit(&SIGNUP_IP, "1.2.3.4").unwrap();
        }
        let one_token = SIGNUP_IP.period.as_secs() / u64::from(SIGNUP_IP.capacity);
        match l.hit(&SIGNUP_IP, "1.2.3.4") {
            Err(AppError::RateLimited(s)) => assert!((1..=one_token).contains(&s), "{s}"),
            other => panic!("{other:?}"),
        }
        l.hit(&SIGNUP_IP, "5.6.7.8").unwrap();
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
        assert!(l.buckets.lock().unwrap().map.len() <= HARD_CAP);
    }

    #[test]
    fn a_flood_of_ips_does_not_wipe_account_budgets() {
        let l = Limiter::with_cap(1_000);
        for _ in 0..5 {
            l.hit(&LOGIN_ACCOUNT_IP, "ana@example.com|1.2.3.4").unwrap();
        }
        assert!(l.hit(&LOGIN_ACCOUNT_IP, "ana@example.com|1.2.3.4").is_err());
        for i in 0..5_000 {
            l.hit(&LOGIN_IP, &format!("10.0.{}.{}", i / 256, i % 256))
                .unwrap();
        }
        let len = l.buckets.lock().unwrap().map.len();
        assert!(len <= 1_000, "{len}");
        assert!(l.hit(&LOGIN_ACCOUNT_IP, "ana@example.com|1.2.3.4").is_err());
        let map = &l.buckets.lock().unwrap().map;
        assert!(map.contains_key(&("login_ip", "10.0.19.135".to_owned())));
        assert!(!map.contains_key(&("login_ip", "10.0.0.0".to_owned())));
    }
}
