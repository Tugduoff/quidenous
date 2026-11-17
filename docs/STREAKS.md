# Streaks

Per member and per room: how many days in a row someone voted. A small flame and a number next to names, the room's own streak next to its name, a party at 7, 30 and 100 days, records in the stats, and an evening push before a streak ends. Feature flag `streaks` in `GET /api/features`.

## What a day is

The server has no daily-question clock: a room opens the next question as soon as everyone voted, optionally paced (`pacing.rs`). So streaks count **calendar days in the member's time zone**: `notification_prefs.tz_offset`, which the app now syncs on start whenever it differs from the phone (before, only with quiet hours on). Someone who never sent one gets the room owner's offset, else UTC. The room streak uses the owner's offset.

For a member in a room, each day is one of:

- **voted**: they voted on a question of the room that day (pick, rank or choose, any of them). The streak goes on.
- **missed**: a question was open that day that they could still vote on (they were in the room, hadn't answered it yet, and weren't skipped by the group on it), and they didn't vote that day. The streak ends.
- **neutral**, anything else: nothing open (room paused, waiting for its owner, out of questions), not in the room yet, only questions they had already answered, or questions the group skipped them on. It neither extends nor breaks the streak.

Refinements:

- **Today never breaks a streak**: until it is over, the streak is the run ending today if I voted today, else the run ending yesterday.
- **Late questions**: a question that opens (or a seat taken) at 20:00 local time or later only counts from the next day. Nobody loses a streak to a question asked at 23:00, after the evening reminder.
- A question removed while open (closed with no votes) never counts. Party questions (with a deadline) never count, and party rooms (`groups.kind = 'party'`) have no streaks at all.
- "Skipped by the group" uses the room's current size for the majority (the past size isn't recorded), like the skip rules.
- Leaving a room and coming back reuses the seat: the days away count as missed when a question was open, so leaving ends a streak.
- A room left with one person still has its question open but nobody can vote: those days count as missed (rare; documented rather than tracked).

**Room streak**: the days (owner's time zone) on which someone had something to vote on, and everyone who could did. People who joined that day, left that day, or were away at its start (docs/ROOMS.md: the last 3 questions closed without them) are not waited for. A day nobody had anything to vote on is neutral.

**Best streak**: the longest run ever, with the same rules.

## Storage

Nothing but the reminder: streaks are computed on the fly from `rounds`, `votes`, `members` and `skips` (every kind of question writes a `votes` row). Rooms are small: measured on the Pi with a room of 5 people, 1,536 questions and 7,500 votes, `GET /streaks` takes 140 ms in an unoptimised debug build (20 ms of it SQL; release builds run the Rust part many times faster). `GET /round/current` and `GET /group` are untouched; the app asks `/streaks` only on a server that lists the feature, and again when a live event refreshes the open question.

Migration 0044: `members.streak_reminded` (local day number, days since 1970-01-01, on which the evening push went out for that seat).

## API

- `GET /api/streaks` (room seat, `X-Room` as usual):
  ```
  { members: { "<member id>": { current, best } }, room: { current, best } | null }
  ```
  Every current member, keyed by member id so the app joins it on its member list. A party answers `{ members: {}, room: null }`. One endpoint rather than fields on the round/group views: the hot views stay as they are and older apps never see it.
- `GET /api/stats` gains `day_streaks` (absent in a party):
  ```
  { member: { member_id, value } | null, room: { current, best } }
  ```
  `member`: the current member with the longest streak ever (2 days or more; ties: the one still going, then the earliest seat). All time, whatever the period. Not to be confused with the existing `streak` / `longest_streak` awards (questions won in a row).
- Notification settings: a new kind `streak` in `off`.

## Evening push

`streaks::spawn_reminder` runs every 5 minutes (`remind_at(state, now)`, `now` injected for tests). Between 19:00 and 20:00 in the member's local time it pushes "Ta série de N jours va s'arrêter" / "Your N-day streak ends tonight" (body: "Vote dans « Salon » avant minuit pour la garder.") when all of these hold:

- the member's current streak in that room is 3 days or more and they haven't voted there today;
- a question is open in the room that they can still vote on (not answered, not skipped by the group; the room has at least 2 people);
- not already sent today for that seat (`members.streak_reminded`, so a restart doesn't resend);
- the room isn't muted, notifications are on, `streak` isn't turned off, and it isn't their quiet hours (also filtered again by `push.rs`, like every push).

No inbox entry: the reminder is only worth something at the moment it is sent.

## App

- **Names**: a Flame and the number next to a member (from 2 days): the room's member list (`group.tsx`) and the people still to vote (Vote tab). Muted, accent colour from 7.
- **Room**: Flame + number next to the room name on the Vote tab (from 2 days).
- **Profile**: under my name, "12 jours d'affilée · record 20" for the current room.
- **Party** at 7, 30 and 100 days for my streak, once per room and milestone (`qdn.streakMilestone.<room>`); the generic party scenes with "jours d'affilée !" and their own punchlines. A room never opened with this version starts from where my streak is, unless it reached a milestone exactly today. Preview: `/?serie=30` on the web.
- **Stats**: a "Jours d'affilée" section with the members' record and the room's best (and the current one when it is going).
- **Notifications**: a "Série en danger" switch, on by default.

Everything shows only when the server lists `streaks`; older apps ignore the new endpoint and field.
