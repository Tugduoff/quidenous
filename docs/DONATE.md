# Buy us a coffee

The app's only money-related feature: an optional link to a donation page, so people who want to can help pay the server. Nothing else changes for anyone, giving or not: no nag, no popup, no badge, nothing unlocked.

Code: `server/src/config.rs` (`DONATION_URL`), `server/src/routes.rs` (`donate`), `app/src/lib/donate.ts`, the `DonateLink` footer in `app/src/app/profile.tsx`. Tests: `server/tests/donate.rs`.

## Server

`DONATION_URL` (deploy/README.md) is a plain URL, so the platform (Ko-fi, Buy Me a Coffee, PayPal, a Stripe payment link, on the shared coflop account) is an ops choice only:

```
DONATION_URL=https://ko-fi.com/<account>
```

- Accepted: `https://<host>…`, no spaces or control characters, no `user@` part, 2048 bytes at most (surrounding spaces trimmed). Anything else is ignored and warned about at start-up (`DONATION_URL is not an https:// URL`): the server starts, the link stays hidden.
- Set: `donate` in `GET /api/features`, and `GET /api/donate` (public, no session) answers `{ "url": "…" }`. Unset: no feature, `404`.
- Changing the platform: edit the env line and restart the server. Apps pick it up on their next start; no app release.

## App

When `donate` is in the features and the build may show it, the profile screen gets a quiet footer between the version line and the privacy / terms links: **Offrir un café ☕** / **Buy us a coffee ☕**, and one line under it ("The app is free, with no ads and no tracking, made by friends. A coffee helps pay for the server."). Tapping opens the page in the system browser (`Linking.openURL`, like the legal links: on the web a new tab).

Where it shows (`DONATE_ALLOWED` in `lib/donate.ts`):

| Build | Link | Why |
| --- | --- | --- |
| Web app / PWA | yes | no store involved |
| APK installed by hand (GitHub Releases) | yes | `EXPO_PUBLIC_DISTRIBUTION=sideload` |
| Google Play bundle | **no** | built without that variable; Play billing policy |
| iOS (any future App Store build) | **no** | `Platform.OS === 'ios'`; App Store guideline 3.1.1 |

`EXPO_PUBLIC_DISTRIBUTION=sideload` is the flag that already tells the APK apart (self-update, `REQUEST_INSTALL_PACKAGES`, `.github/workflows/android.yml`). It is inlined at build time, and the OTA bundles follow it: the APK's JS is exported with it, the Play bundle's without it, and the two builds have different runtimes, so an OTA update can never carry the link into the Play app.

## Store policies

- **Google Play.** The Payments policy requires Play billing for digital goods and services in Play-distributed apps and forbids leading users to another payment method for them. Donations through an outside page are only allowed for validated tax-exempt organizations (a 501(c)(3) or the local equivalent). Google enforces this on tip links to the developer: it told AnkiDroid to remove its donation link or use Play billing. Coflop is not a charity, so the Play build shows no link (Play billing for tips would cost 15 % and an in-app product for no real gain).
- **Apple.** Guideline 3.1.1: tips and donations to the developer tied to an app go through in-app purchase; 3.2.1(vi) / 3.2.2 only let approved nonprofits collect outside it. Any App Store build hides the link the same way.
- **Web and sideloaded APK.** No store, no store billing rule. The page itself belongs to the platform chosen (its own terms and privacy policy); the privacy page lists it among third parties.

If the policies loosen (US link-out rulings, EU DMA alternatives) showing it in a store build is a one-line change in `DONATE_ALLOWED`, to be weighed against each store's link-out fees and paperwork.
