# Mascots: la bande

Five friends drawn from one shared part library (the "Petons" rig): **Zeste** (lime, the instigator), **Bulle** (blue, the shy one
who always gets picked), **Pavé** (orange, the know-it-all), **Paillette** (pink, the drama queen), **Roupi** (violet, the sleepy
one who never stands up). JS only: they ship over the air (react-native-svg + reanimated, no new native module).

## Files
- `app/scripts/mascot-parts.mjs`: the rig, source of truth (bodies, parts, moods, quirks). Writes `app/src/lib/mascot-parts.json`
  and its byte-identical server copy `server/assets/mascot.json` (for share cards later; checked by a test in `server/src/avatar.rs`
  and by `node scripts/mascot-parts.mjs --check` in CI). Run it from `app/` after editing.
- `app/src/lib/mascot.ts`: types, `ROLE`, `resolvePose()`, `compose(who, mood | pose, { lod, prop, tint })` → cached draw groups.
  Tests: `bun test src/lib/mascot.test.ts` (from `app/`).
- `app/src/components/mascot.tsx`: `<Mascot who mood size animate prop tint motion />`, decorative (hidden from screen readers).
- `app/src/components/mascot-band.tsx`: the group shot (everyone points at Bulle), the only place with several mascots.

## Drawing
Every character = a body (silhouette + face, arm, leg and prop anchors) + parts (eyes, brows, mouth, arms, legs, props), all in a
200×200 box. Left limbs are the right ones mirrored; legs pick the stance and lift the whole body (hop, tiptoe, sit). Colours are
tokens resolved per character; the ink outline stays near-black in both themes, the ground shadow follows the theme.
Under 48 px (`lod: 'lo'`) the drawing drops brows, gloss, inner shading, limb joints and small highlights: 19–36 SVG elements
instead of 32–49.

## Motion
One shared clock (a looping shared value, UI thread) drives every live mascot: breathing, blinking (open eyes only, a double blink
now and then), and per mood a sway (happy), lunge toward the pointing finger (accuse), slow tilt + floating zzz (sleepy), double hop
with a shrinking shadow (celebrate), slow sink + dripping tears (sad), foot-tap bob + wobbling "?" (think), shiver (caught).
A mood change squashes the body while the parts swap. No per-frame JS, no re-render: the drawing re-renders only when the mood or
the size class changes. Static under 40 px, under reduced motion and while the screen is not focused; the clock stops when no
live mascot is mounted.

## Where (one per screen, never in lists, forms, settings, chat or on the question card)
| Moment | Who |
|---|---|
| Sign-in hero, party TV lobby | the band |
| Alone in the room | Zeste waving |
| No more questions, update required | Pavé thinking |
| Waiting for votes (≤ 5 still to vote) | Roupi asleep |
| History empty | Roupi asleep |
| Stats empty ("le calme avant la tempête") | Roupi content |
| Offline / load failed (`QueryState`) | Paillette in tears |
| Crash (error boundary) | Paillette knocked flat |
| Streak milestones, party over (phone and TV) | Paillette hopping |
| Pinned result summary | Zeste pointing (winner), Paillette (best guesser), Pavé (nobody guessed) |
