use std::fmt;

use serde::{Deserialize, Serialize};

macro_rules! ids {
    ($($name:ident),* $(,)?) => {$(
        #[derive(
            Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, sqlx::Type,
        )]
        #[serde(transparent)]
        #[sqlx(transparent)]
        pub struct $name(i64);

        impl $name {
            pub const fn from_raw(raw: i64) -> Self {
                Self(raw)
            }

            pub const fn raw(self) -> i64 {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }
    )*};
}

ids!(
    UserId,
    RoomId,
    MemberId,
    RoundId,
    QuestionId,
    MessageId,
    PackId,
    SuggestionId,
    InviteId,
);
