import { useIsFocused } from 'expo-router';
import * as Linking from 'expo-linking';
import { useEffect, useState, useSyncExternalStore } from 'react';
import { Platform } from 'react-native';
import { openedFromNotification } from '@/lib/device/native-push';
import { storage } from '@/lib/device/storage';
import { plainStart } from '@/lib/device/launch-params';
import { useRooms } from '@/lib/api/rooms';
import { useFeatures } from '@/lib/api/features';

/** '1': last known, the Today tab is there (feature on, 2+ rooms); '0': it isn't. Read before the first screen. */
const TAB_KEY = 'qdn.today';
/** The session's own key: signed in when the app started (not signing in now, which is navigating). */
const TOKEN_KEY = 'qdn.token';

let saved: string | null = null;
let signedInAtStart = false;
let loaded = false;
const listeners = new Set<() => void>();
Promise.all([storage.get(TAB_KEY), storage.get(TOKEN_KEY)])
  .then(([tab, token]) => {
    saved = tab;
    signedInAtStart = !!token;
  })
  .catch(() => {})
  .finally(() => {
    loaded = true;
    listeners.forEach((l) => l());
  });
const subscribe = (l: () => void) => {
  listeners.add(l);
  return () => void listeners.delete(l);
};

function remember(on: boolean) {
  const value = on ? '1' : '0';
  if (saved === value) return;
  saved = value;
  storage.set(TAB_KEY, value).catch(() => {});
  listeners.forEach((l) => l());
}

/**
 * Whether the Today tab exists: the server has it and I'm in two rooms or more (parties aside). With a single room
 * it would be a one-card detour before the vote. Until the server answers, the last known answer (no tab pops in or out
 * at start); every answer is remembered for the next start.
 */
export function useTodayTab(): boolean {
  const features = useFeatures();
  const { data: rooms } = useRooms();
  const last = useSyncExternalStore(subscribe, () => saved);
  const featuresKnown = features.query.data != null;
  const on = featuresKnown && features.today && rooms ? rooms.filter((r) => r.kind !== 'party').length >= 2 : null;
  const known = featuresKnown && (!features.today || rooms != null);
  useEffect(() => {
    if (known) remember(!!on);
  }, [known, on]);
  return known ? !!on : last === '1';
}

/**
 * The app was opened plainly (its icon, the bare web address), not by a notification, a `?room=` link or another link:
 * those open their own screen and keep it.
 */
function plainLaunch() {
  if (openedFromNotification()) return false;
  if (Platform.OS === 'web') return plainStart();
  const url = Linking.getLinkingURL();
  if (!url) return true;
  const { path, queryParams } = Linking.parse(url);
  return !path?.replace(/^\/+|\/+$/g, '') && !Object.keys(queryParams ?? {}).length;
}

/** The vote screen has shown once: from then on it's the user's (and every link's) to navigate. */
let landed = false;

/**
 * Called by the vote screen (the `/` route) on its first render: true when the app should open on Today instead,
 * i.e. at a plain cold start, signed in from the start, with the Today tab known from last time. The vote screen then
 * redirects straight away (it draws nothing meanwhile). Once and for all: never again after that first render.
 */
export function useLandOnToday() {
  const tab = useTodayTab();
  const focused = useIsFocused();
  const [land, setLand] = useState(() => !landed && loaded && signedInAtStart && saved === '1' && plainLaunch());
  useEffect(() => {
    landed = true;
  }, []);
  useEffect(() => {
    if (!land) return;
    // Left for Today (the vote screen is itself again behind it), or the tab went away meanwhile (no redirect possible);
    // else the redirect didn't happen (shouldn't): never leave the vote screen blank.
    const giveUp = setTimeout(() => setLand(false), !focused || !tab ? 0 : 500);
    return () => clearTimeout(giveUp);
  }, [land, focused, tab]);
  return land && focused && tab;
}
