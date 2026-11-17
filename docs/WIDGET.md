# Home-screen widget

Android home-screen widget with the room's question of the day: whether I voted, how many votes are still missing, my streak flame, and a mascot for the state. A tap opens the app on the vote (or on the result). French and English, light and dark. iOS: design only (end of this page).

## What it shows

| State (`GET /api/widget` → `state`) | Big line | Status | Mascot | Tap opens |
|---|---|---|---|---|
| `result`: a result I voted on and haven't seen | that question | "Le résultat est tombé !" (pink), or "N résultats à voir"; medium adds "Une nouvelle question t'attend" when the next one is open and unvoted | Paillette, celebrating | `quidenous://round/<id>?room=<room>` |
| `vote`: the open question waits for me | the question | "À toi de voter !" (lime) | Zeste, pointing at the question | `quidenous://?room=<room>` |
| `wait`: I voted, the room hasn't | the question | "Plus que N votes" | Roupi, asleep | same |
| `idle`: no open question (paused room, out of questions) | "Pas de question pour l'instant" | "Prochaine à 21:30" when it opens by itself | Pavé, thinking | same |
| `no_room`, signed out | "Ouvre qui de nous ?" | "Rejoins un salon pour jouer" / "Connecte-toi pour jouer" | Zeste, waving | `quidenous://` |

The header says *qui de nous ?* (Bricolage Grotesque) plus the room's name (medium only) and the streak: lucide's flame and my day streak in that room (docs/STREAKS.md), in the accent colour once I voted today, muted while it's at risk. No flame at a party or at 0.

`?room=` switches the app to that room on any screen (`useRoomParam`, lib/rooms.ts), like a tapped notification.

Two widgets in the picker, same drawing: **QdnSmall** (2×2, 110 dp min) and **QdnMedium** (4×2, 250 dp min); both resize, the layout follows the width (under 250 dp: small). The language is the account's (`users.locale`, in the answer); signed out, the phone's. Both themes are drawn on every update and the launcher shows the one the phone uses.

## How it's built

- **Library: [react-native-android-widget](https://github.com/sAleksovski/react-native-android-widget) 0.26.0**, an Expo config plugin (no ejecting, CNG as before). Chosen over Expo's own `expo-widgets` (SDK 58 does Android since September, behind `enableAndroid`): that one runs a separate JS bundle on Jetpack Glance with 18 components, no SVG, no custom font, no preview image, no update period and no documented deep-link tap, and is weeks old on Android. This library draws our JSX (flex boxes, text, SVG) into a bitmap from the app's own bundle, supports fonts, light + dark renders, `OPEN_URI` taps, `updatePeriodMillis` and picker previews. 0.26.0 is the first release that works with Expo 58 (older ones crash release builds under AGP 9's R8, issue #159). Voltra (Callstack) is the other credible option; not needed for this.
- **Code** (`app/src/widget/`): `widget.tsx` the drawing (`'use no memo'`: the React Compiler breaks the library's plain-function components), `art.ts` the five mascots as static SVG (generated from the Petons rig by `scripts/widget-art.mjs`, PR #83), `state.ts` data and tokens, `task.tsx` the background side, `task.web.ts` a no-op for the web. Strings in `locales/{fr,en}/widget.ts` through `lib/i18n-core.ts` (the translation tables without native dependencies).
- **Entry**: `app/index.ts` (package.json `main`) loads Expo Router then registers the widget task and the push task: Android starts them with no screen.
- **Native**: `app.json` plugin entry (widgets, fonts Bricolage 800 + Inter 600/700, preview images `assets/widget/preview-*.png`), plus `expo-task-manager` for the push task. **This changes the native fingerprint: it needs a new APK / Play upload, no OTA.** Once installed, changes to the widget's drawing and data are JS and ship over the air like the rest.
- **Permissions**: the library declares none itself. WorkManager (its update engine) brings `WAKE_LOCK`, `ACCESS_NETWORK_STATE`, `RECEIVE_BOOT_COMPLETED` and `FOREGROUND_SERVICE` through manifest merging; the CI step checking the Play bundle still rejects the four blocked ones (`blockedPermissions` unchanged).

## Data flow

The widget never needs the app open.

1. **Token.** The widget task reads the session from the secure store (`qdn.token`, `qdn.server`, `qdn.room`, the same keys as `lib/session.tsx`) and trades it once for a **widget token**: `POST /api/widget/token {fcm_token?}` (session bearer) → `{token: "w_…"}`, saved as `qdn.widget.token` with the session it came from (another session → minted again). It only works on `GET /api/widget`; session tokens don't work there. Stored hashed (`widget_tokens`, migration 0054) with a foreign key to the session (`ON DELETE CASCADE`): sign-out, "sign out everywhere", password change, suspension, account deletion and the 180-day idle purge all end it. One per session: minting again replaces it. `DELETE /api/widget/token` (widget bearer) when the last widget leaves the home screen or the app forgets the session on this phone.
2. **Read.** `GET /api/widget?room=<id>` → `{state, lang, room{id,name,kind}, round{id,number,question,kind}, voted, votes, needed, result, unseen, streak{current,today}, next_open_at}`. No `room`, or a room I left: the room I used last. In no room: `state: "no_room"`. `Cache-Control: private, max-age=60`. Cheap: a handful of indexed queries plus the streak computation of `GET /streaks`. A glance at the widget is no visit: it doesn't touch the session's "last used", the member's activity or "seen" markers. The last answer is kept (`qdn.widget.cache`) and shown when offline.
3. **Refresh**, battery first:
   - **The app**: when the session, the room or the stage changes, and each time the app goes to the background (a vote or a seen result shows on the home screen at once). Only if a widget exists (`getWidgetInfo`).
   - **Silent push**: when a question opens or closes in a room (`notify_round`, parties too), the server sends a data-only FCM message `{widget: "1", room}` to the widget tokens of the room's members that registered an FCM token: no notification shown, `priority: normal` (delivered at the phone's next wake-up, no Doze wake), `collapse_key: widget-<room>` (one pending per room), `ttl: 1h`. The app's `expo-notifications` background task (`registerTaskAsync`) redraws the widgets. A vote alone pings nobody (the count catches up on the next refresh). Dead FCM tokens are dropped. Needs no notification permission.
   - **Timer**: `updatePeriodMillis` = 1 hour per widget (the OS floor is 30 min; hourly is the documented upper bound for a widget that wakes the phone).
   - Resizing redraws from the last answer, no request. Several widgets and a push arriving together share one request.

## Previews

No emulator on the Pi: `bun scripts/widget-preview.mjs [out.html]` (from `app/`) renders the real `widget.tsx` for sample states, both themes and both sizes as HTML (stand-ins for the native modules; flex boxes as CSS), to screenshot in any browser. Close to the phone, not pixel-exact: Android lays it out with LinearLayouts and draws text itself. The picker previews `assets/widget/preview-small.png` / `preview-medium.png` come from it (dark, "À toi de voter").

## Testing on a phone

Install the APK from the `android` workflow, long-press the home screen → Widgets → *qui de nous ?*. Then: vote in the app and leave it (wait state at once), have someone else close the question (result within a few minutes when the phone is awake), sign out (signed-out state). `adb shell cmd appwidget` / `adb shell dumpsys appwidget` lists them.

## iOS later (WidgetKit), not built

The same `GET /api/widget` feeds a WidgetKit extension:

- **Token**: the app mints the widget token (as the Android task does) and writes it to a shared **App Group** keychain item (`kSecAttrAccessGroup` = the group) the extension can read. Only the narrow widget token ever leaves the app's own keychain: the extension can't vote, chat or change the account.
- **Timeline**: a `TimelineProvider` fetches `/api/widget` in `getTimeline` and returns one entry with `.after(now + 30 min)` (`idle` with `next_open_at`: an entry at that time too). The app calls `WidgetCenter.shared.reloadAllTimelines()` where Android calls `refreshWidgets()` (session/room change, going to the background).
- **Push**: iOS has no silent wake for widgets from FCM data like Android; the server would add, for iOS devices, either a background APNs push (`content-available: 1`, ≤ 2-3 an hour, then the app reloads timelines) or WidgetKit push updates (iOS 26 `WidgetPushHandler`, APNs `widgets` topic), keyed by the same `widget_tokens` row (an `apns_token` column next to `fcm_token`).
- **Drawing**: SwiftUI `systemSmall` / `systemMedium`, the same states, strings (shipped in the extension's `Localizable.strings`, picked by `lang`) and mascots (`art.ts` as SVG assets or pre-rendered PDFs), `widgetURL` with the same `quidenous://` deep links. `expo-widgets` (`createWidget`, iOS side mature) or a config plugin adding the extension target would build it without ejecting.
