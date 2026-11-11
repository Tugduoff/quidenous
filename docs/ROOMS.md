# Rooms, accounts, permissions, friends

Status: design, 2026-11-11. Built in phases so the live group never breaks and old app versions keep working.

## Model

- **users**: one per person: `id, name, color, photo, friend_code, created_at`. The token identifies a user (`user_tokens`).
- **groups** (= rooms): `name, invite_code (rotatable), owner via members.role, settings`.
- **members**: a user in a room: `user_id, group_id, role ('owner' | 'member'), removed_at`. Votes, nudges, pins, skips keep pointing at members, so history is untouched.
- **friends**: `user_id, friend_id`. Anyone you share a room with counts as a friend; a friend code or QR adds someone new.

## Requests

- Token = user. Header `X-Room: <group_id>` picks the room; without it, the user's most recently used room (that is what 0.6.x apps do, so they keep working on the main group).
- The `Me` extractor still yields the member for that room, so every existing endpoint stays as is.
- Live socket: one per user, carries events of all their rooms (each event tagged with its room).
- Push: subscriptions move from member to user; a room notification goes to its members' users.

## Migration of the existing group

- Every current member becomes a user (same name, color, photo); `member_tokens` become `user_tokens` with the same hashes, so nobody is signed out.
- Owner of the main group: its earliest member. Ownership can be handed over.
- `JOIN_CODE` stays the invite code of the main group.

## Rooms and permissions

| Action | Who |
| --- | --- |
| Create a room | anyone (becomes owner) |
| Invite (link, code, QR), add a friend | any member (owner can restrict) |
| Rotate the invite code | owner |
| Rename, settings | owner |
| Vote to skip / remove | members, if the owner leaves it on |
| Remove someone directly, no vote (force kick) | owner |
| Hand over ownership, delete the room | owner |
| Leave | anyone; an owner leaving hands over to the oldest member, or deletes the room if alone |

Settings per room: majority skip on/off, majority removal on/off, who can add or import questions (everyone / owner).

## Phases

1. Server: users, memberships, `X-Room`, migration, push and live per user. No visible change.
2. Server: create / join / leave rooms, invites, owner actions, settings.
3. App: room switcher, create and join screens, group page with invite link and owner settings.
4. Friends: friend code and QR, add friends to a room in one tap, "X added you to Y" notification.
