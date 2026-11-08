# Qui de nous ? — detailed plan (v1)

Private app for one group of friends. A question appears ("Who would…?"), everyone votes for a member,
votes are public, the next question unlocks as soon as everyone has voted. History tab with all results.

## Decisions
- App: Expo (SDK 57) + expo-router, TypeScript. Android APK, iPhone via the web build (PWA) unless we pay Apple.
- Backend: Rust (Axum + sqlx/SQLite + WebSocket), Docker image, hosted on Ravenne's homelab behind HTTPS.
- Votes are public. A question only closes when every member has voted (no timeout).
- One group at first (code-based join); the schema allows several groups later.

## Screens
1. Welcome / join: first name, photo (optional), group code.
2. Vote (home): question card, grid of members (photo + name, not yourself), "3/5 have voted" with avatars.
   After voting: "waiting for X, Y" + list of who already voted (not for whom until the question closes).
3. Result (shown when the last vote lands, then auto "Next question" button):
   bars per member with % and the avatars of who voted for them, winner highlighted.
4. History: list of closed questions (question, winner + avatar, date) → detail = full result.
5. Questions: add your own question, see the queue; 👍/👎 on a result to flag bad questions.
6. Profile/settings: change photo/name, leave group.

## Data model (SQLite)
- groups(id, name, join_code, created_at)
- members(id, group_id, name, photo_path, token_hash, created_at)
- questions(id, group_id NULL = built-in deck, text, author_id NULL, created_at)
- rounds(id, group_id, question_id, opened_at, closed_at NULL)
- votes(round_id, voter_id, target_id, created_at, PK(round_id, voter_id))
- ratings(round_id, member_id, value ±1)

Rule: one open round per group. When votes == members count → close round, open the next unseen question
(group questions first, then the built-in deck, random order), broadcast both events.

## API (JSON, Bearer token)
- POST /join {code, name} → {member, token}; PUT /me {name}; PUT /me/photo (multipart, resized to 256 px)
- GET /group → members
- GET /round/current → question, voters so far, my vote, result if closed
- POST /round/current/vote {target_id} (409 if already voted / self vote)
- GET /history?cursor= → closed rounds; GET /rounds/:id → full result
- POST /questions {text}; GET /questions/queue; POST /rounds/:id/rating {value}
- WS /live → events: vote_cast {voter}, round_closed {result}, round_opened {question}, member_joined

## App structure
- Tabs: Vote · History · Questions; profile from the avatar.
- State: TanStack Query + WebSocket that invalidates queries; token in SecureStore (localStorage on web).
- Design: own identity (not AskUs): dark, bold, one accent colour, big question typography, avatar-heavy.

## Built-in deck
~150 questions written by Pivot: extreme "who would…", dilemmas and spicy group questions, in French.
Reviewed by the group before launch.

## Delivery
1. Backend: models, join, round loop, votes, WS, history (+ tests). Docker + compose for Ravenne.
2. App: join, vote, live updates, result, history. Web build served by the backend (PWA for iPhones).
3. Questions tab, photos, ratings, deck.
4. Android APK + iPhone install, then polish.

## iPhone install
- Free: web version (PWA) served by the server, "Add to Home Screen" in Safari. Same app, full screen.
- Native: Apple developer account (€99/year) → EAS internal build (register each iPhone) or TestFlight.
- No way to install an .ipa on a normal iPhone without Apple signing (free sideloading expires every 7 days).
