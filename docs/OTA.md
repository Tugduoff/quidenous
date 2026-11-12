# Over-the-air updates

JS-only changes (screens, texts, animations, fixes) reach phones without a new APK: [expo-updates](https://docs.expo.dev/versions/latest/sdk/updates/) talking to our own server ([protocol v1](https://docs.expo.dev/technical-specs/expo-updates-1/)), no paid service.

## How it works

- **On the phone.** At each launch the native side asks `https://qdn.coflop.fr/api/updates/manifest` for the newest update of its runtime and downloads it in the background (`checkAutomatically: ON_LOAD`, `fallbackToCacheTimeout: 0`: the launch never waits). Coming back to the foreground checks again, at most every 30 min (`app/src/lib/ota.ts`). A downloaded update applies on the next cold start; meanwhile a small toast offers *Relancer*. If a new bundle crashes at launch, expo-updates falls back to the previous one by itself.
- **Runtime version = native fingerprint** (`"runtimeVersion": { "policy": "fingerprint" }`). It hashes everything native: native dependencies and their versions, the native parts of the app config (plugins, permissions, icons…), not `version`/`versionCode`. A phone only receives bundles exported for exactly its native build, so a JS bundle can never land on an APK that lacks a native module it needs. (`appVersion` would rely on us bumping the version for every native change and would also split runtimes on every JS-only release; fingerprint is automatic and safe.) The APK and the Play bundle have different fingerprints (the APK has the self-update permission), so each gets its own updates and the APK's self-updater JS never reaches Google Play.
- **Code signing.** Every manifest/directive is signed in CI with an RSA key (`UPDATES_PRIVATE_KEY` GitHub secret; a copy in `~/.config/quidenous/ota-keys/` on the Pi, keep a backup). The app embeds the certificate (`app/certs/certificate.pem`, valid until 2051) and refuses anything unsigned or signed by another key. The server never holds the key: it stores the signed bytes and serves them verbatim, so a compromised server cannot push code to phones.
- **Server** (`server/src/updates.rs`): `GET /api/updates/manifest` (headers `expo-platform`, `expo-runtime-version`; multipart answer with the manifest or a directive, `204` when there is nothing), `GET /api/updates/assets/<runtime>/<id>/<file>`, and with `Authorization: Bearer $UPDATES_PUBLISH_TOKEN`: `POST /api/updates/publish` (a tar.gz, see `app/scripts/ota-publish.mjs`), `GET /api/updates` (list), `DELETE /api/updates/<runtime>/<id>`. Updates live in `DATA_DIR/updates/<runtime>/<id>/`; the last 5 per runtime are kept. Without `UPDATES_PUBLISH_TOKEN` publishing is off (manifests still answer `204`).
- **CI** (`.github/workflows/android.yml`): every push to `main` touching `app/` builds the APK and the AAB, then exports the JS (`npx expo export --platform android`) and publishes it twice, once per build, under the runtime read from the build itself (`assets/fingerprint`). The update is dated like the build's embedded bundle, so a phone installing that very APK doesn't download it again, while every older build of the same runtime does. When a commit changes the fingerprint, its update simply starts a new runtime that no phone has yet: those phones need the APK release (the in-app APK updater offers it).

## When an APK is still needed

Anything native: a new or upgraded package with native code (`npx expo install …` of an `expo-*`/`react-native-*` module), an Expo SDK upgrade, `app.json` plugins, permissions, icons, splash, package name, `google-services.json`. You don't have to decide: if the fingerprint changed, the OTA goes to a new runtime and only the APK release reaches existing phones. JS-only releases can still be released as an APK too; phones that already got the update over the air are not offered that APK again (the version shown and compared is the one of the running JS).

## Rolling back

- **Back to an older JS**: revert the bad commit on `main` and push. The new update is newer, phones take it at their next launch.
- **Emergency brake**: run the *ota-rollback* workflow (Actions → ota-rollback → Run workflow; leave *runtime* empty for all). It publishes a signed `rollBackToEmbedded` directive: phones go back to the bundle inside their APK at their next launch, until a newer update is published. From the Pi: `cd app && UPDATES_PUBLISH_TOKEN=… UPDATES_PRIVATE_KEY="$(cat ~/.config/quidenous/ota-keys/private-key.pem)" node scripts/ota-publish.mjs --rollback-embedded`.
- `DELETE /api/updates/<runtime>/<id>` only stops *new* downloads of an update (phones that have it keep it): prefer the two above.

## Checking it

```bash
curl -si https://qdn.coflop.fr/api/updates/manifest -H 'expo-platform: android' -H 'expo-runtime-version: <fingerprint>' -H 'expo-protocol-version: 1'
curl -s https://qdn.coflop.fr/api/updates -H "Authorization: Bearer $UPDATES_PUBLISH_TOKEN"
```

The fingerprint of an APK: `unzip -p app-release.apk assets/fingerprint`. The CI log of the *android* workflow prints what it published.

## Google Play

Allowed: Play's policy permits code downloaded into a JS runtime as long as it doesn't change the app's purpose (Device and Network Abuse policy, interpreted-language exception). Updates stay on in both the APK and the AAB; only the APK self-update (a different thing, `src/lib/update.ts`) is off on Play.

## Server setup

One env var on the server: `UPDATES_PUBLISH_TOKEN` (the same value as the GitHub secret). Behind nginx, `client_max_body_size` must allow the upload (about 5 MB today; 10m is enough). Updates are stored in `DATA_DIR/updates/`: about 15 MB each, 5 kept per build, so ~150 MB at most.
