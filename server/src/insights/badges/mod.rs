use crate::platform::i18n::Lang;

pub struct BadgeDef {
    pub key: &'static str,
    pub fr: &'static str,
    pub en: &'static str,
    pub icon: &'static str,
    pub mascot: &'static str,
    pub tiers: [i64; 3],
}

impl BadgeDef {
    pub(crate) fn name(&self, lang: Lang) -> &'static str {
        lang.pick(self.fr, self.en)
    }

    pub fn tier_for(&self, value: i64) -> i64 {
        self.tiers.iter().filter(|t| value >= **t).count() as i64
    }
}

const fn def(
    key: &'static str,
    fr: &'static str,
    en: &'static str,
    icon: &'static str,
    mascot: &'static str,
    tiers: [i64; 3],
) -> BadgeDef {
    BadgeDef {
        key,
        fr,
        en,
        icon,
        mascot,
        tiers,
    }
}

pub const CATALOGUE: [BadgeDef; 14] = [
    def(
        "pilier",
        "Le pilier",
        "The pillar",
        "landmark",
        "pave",
        [50, 250, 1000],
    ),
    def(
        "suspect",
        "Le suspect habituel",
        "The usual suspect",
        "fingerprint",
        "bulle",
        [5, 25, 100],
    ),
    def(
        "evidence",
        "L'évidence",
        "The obvious one",
        "target",
        "bulle",
        [1, 5, 20],
    ),
    def(
        "devin",
        "Le devin",
        "The oracle",
        "eye",
        "pave",
        [3, 15, 50],
    ),
    def(
        "flair",
        "Le flair",
        "The nose",
        "radar",
        "zeste",
        [25, 100, 400],
    ),
    def(
        "incorruptible",
        "L'incorruptible",
        "The incorruptible",
        "shield-check",
        "pave",
        [30, 150, 500],
    ),
    def(
        "narcisse",
        "Narcisse",
        "Narcissus",
        "flower-2",
        "paillette",
        [5, 20, 50],
    ),
    def(
        "marathon",
        "Le marathonien",
        "The marathoner",
        "flame",
        "paillette",
        [7, 30, 100],
    ),
    def(
        "gachette",
        "La gâchette",
        "Quick draw",
        "zap",
        "zeste",
        [10, 50, 200],
    ),
    def(
        "plume",
        "La plume",
        "The quill",
        "feather",
        "pave",
        [3, 15, 50],
    ),
    def(
        "bavard",
        "Le bavard",
        "The chatterbox",
        "message-circle",
        "paillette",
        [50, 250, 1000],
    ),
    def(
        "relou",
        "Le relou",
        "The nag",
        "bell-ring",
        "zeste",
        [10, 50, 200],
    ),
    def(
        "couche_tard",
        "Le couche-tard",
        "The night owl",
        "moon",
        "roupi",
        [10, 50, 200],
    ),
    def(
        "fetard",
        "Le fêtard",
        "The party animal",
        "party-popper",
        "paillette",
        [1, 5, 20],
    ),
];

pub(crate) type Metrics = [i64; CATALOGUE.len()];

pub fn find(key: &str) -> Option<&'static BadgeDef> {
    CATALOGUE.iter().find(|b| b.key == key)
}

pub(crate) fn tier_name(tier: i64, lang: Lang) -> &'static str {
    match tier {
        3 => lang.pick("or", "gold"),
        2 => lang.pick("argent", "silver"),
        _ => "bronze",
    }
}

pub(crate) fn hint(key: &str, n: i64, lang: Lang, done: bool) -> String {
    let s = if n > 1 { "s" } else { "" };
    let fr = |next: String, got: String| if done { got } else { next };
    match lang {
        Lang::Fr => match key {
            "pilier" => fr(format!("Vote {n} fois"), format!("{n} votes et plus")),
            "suspect" => fr(
                format!("Gagne {n} questions"),
                format!("{n} questions gagnées et plus"),
            ),
            "evidence" => fr(
                format!("Récolte toutes les voix (3 ou plus) {n} fois"),
                format!("Toutes les voix pour toi, {n} fois et plus"),
            ),
            "devin" => fr(
                format!("Gagne {n} paris au Tripot"),
                format!("{n} paris gagnés au Tripot et plus"),
            ),
            "flair" => fr(
                format!("Vote {n} fois pour le gagnant (pas pour toi)"),
                format!("{n} votes pour le gagnant et plus"),
            ),
            "incorruptible" => fr(
                format!("Vote {n} fois sans jamais voter pour toi"),
                format!("{n} votes, jamais pour toi"),
            ),
            "narcisse" => fr(
                format!("Vote {n} fois pour toi-même"),
                format!("{n} votes pour toi-même et plus"),
            ),
            "marathon" => fr(
                format!("Vote {n} jours d'affilée dans un salon"),
                format!("{n} jours d'affilée et plus"),
            ),
            "gachette" => fr(
                format!("Vote en premier sur {n} questions"),
                format!("Premier à voter, {n} fois et plus"),
            ),
            "plume" => fr(
                format!("Écris {n} questions qui sont posées"),
                format!("{n} questions écrites et posées et plus"),
            ),
            "bavard" => fr(
                format!("Envoie {n} messages dans le chat"),
                format!("{n} messages et plus"),
            ),
            "relou" => fr(
                format!("Relance les autres {n} fois"),
                format!("{n} relances et plus"),
            ),
            "couche_tard" => fr(
                format!("Vote {n} fois entre 23 h et 5 h"),
                format!("{n} votes nocturnes et plus"),
            ),
            "fetard" => fr(
                format!("Vote dans {n} soirée{s}"),
                format!("{n} soirée{s} et plus"),
            ),
            _ => String::new(),
        },
        Lang::En => match key {
            "pilier" => fr(format!("Vote {n} times"), format!("{n}+ votes")),
            "suspect" => fr(format!("Win {n} questions"), format!("{n}+ questions won")),
            "evidence" => fr(
                format!("Get every vote (3 or more) {n} time{s}"),
                format!("Every vote for you, {n}+ times"),
            ),
            "devin" => fr(
                format!("Win {n} Tripot bets"),
                format!("{n}+ Tripot bets won"),
            ),
            "flair" => fr(
                format!("Vote for the winner {n} times (not yourself)"),
                format!("{n}+ votes for the winner"),
            ),
            "incorruptible" => fr(
                format!("Vote {n} times, never for yourself"),
                format!("{n}+ votes, never for yourself"),
            ),
            "narcisse" => fr(
                format!("Vote for yourself {n} times"),
                format!("{n}+ votes for yourself"),
            ),
            "marathon" => fr(
                format!("Vote {n} days in a row in a room"),
                format!("{n}+ days in a row"),
            ),
            "gachette" => fr(
                format!("Vote first on {n} questions"),
                format!("First to vote, {n}+ times"),
            ),
            "plume" => fr(
                format!("Write {n} questions that get asked"),
                format!("{n}+ questions written and asked"),
            ),
            "bavard" => fr(format!("Send {n} chat messages"), format!("{n}+ messages")),
            "relou" => fr(
                format!("Nudge the others {n} times"),
                format!("{n}+ nudges"),
            ),
            "couche_tard" => fr(
                format!("Vote {n} times between 11 pm and 5 am"),
                format!("{n}+ late-night votes"),
            ),
            "fetard" => fr(
                format!("Vote at {n} part{}", if n > 1 { "ies" } else { "y" }),
                format!("{n}+ parties"),
            ),
            _ => String::new(),
        },
    }
}

pub(crate) fn inbox_text(def: &BadgeDef, tier: i64, lang: Lang) -> String {
    match lang {
        Lang::Fr => format!(
            "Nouveau titre : {} ({})",
            def.name(lang),
            tier_name(tier, lang)
        ),
        Lang::En => format!("New title: {} ({})", def.name(lang), tier_name(tier, lang)),
    }
}

macro_rules! mine {
    () => {
        "(SELECT value FROM json_each(?1))"
    };
}

mod award;
mod handlers;
pub use award::*;
pub(crate) use handlers::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogue_is_consistent() {
        let mut keys: Vec<&str> = CATALOGUE.iter().map(|b| b.key).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), CATALOGUE.len());
        for b in &CATALOGUE {
            assert!(
                b.tiers[0] < b.tiers[1] && b.tiers[1] < b.tiers[2],
                "{}",
                b.key
            );
            assert!(
                ["zeste", "bulle", "pave", "paillette", "roupi"].contains(&b.mascot),
                "{}",
                b.key
            );
            for lang in Lang::ALL {
                for n in [1, 2, b.tiers[2]] {
                    assert!(!hint(b.key, n, lang, false).is_empty(), "{}", b.key);
                    assert!(!hint(b.key, n, lang, true).is_empty(), "{}", b.key);
                }
            }
        }
    }

    #[test]
    fn tiers_and_texts() {
        let pilier = find("pilier").unwrap();
        assert_eq!(pilier.tier_for(0), 0);
        assert_eq!(pilier.tier_for(49), 0);
        assert_eq!(pilier.tier_for(50), 1);
        assert_eq!(pilier.tier_for(250), 2);
        assert_eq!(pilier.tier_for(5000), 3);
        assert_eq!(
            inbox_text(pilier, 3, Lang::Fr),
            "Nouveau titre : Le pilier (or)"
        );
        assert_eq!(
            inbox_text(pilier, 3, Lang::En),
            "New title: The pillar (gold)"
        );
        assert_eq!(hint("pilier", 250, Lang::Fr, false), "Vote 250 fois");
        assert_eq!(hint("pilier", 250, Lang::En, false), "Vote 250 times");
        assert_eq!(hint("fetard", 1, Lang::En, false), "Vote at 1 party");
        assert!(TitleChip::new("nope", 1, Lang::Fr).is_none());
        assert!(TitleChip::new("pilier", 0, Lang::Fr).is_none());
    }
}
