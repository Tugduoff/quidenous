# App (Expo SDK 58)

```bash
npm install
EXPO_PUBLIC_API_URL=http://<server>:8080 npx expo start   # phone: Expo Go / dev build
npx expo export --platform web                            # web build, served by the Rust server (WEB_DIR)
npx tsc --noEmit
```

Screens: `join`, tabs `(tabs)/index` (vote → waiting → result), `history`, `questions`, `round/[id]`.
Live updates: `src/lib/live.ts` (WebSocket → React Query invalidation).
