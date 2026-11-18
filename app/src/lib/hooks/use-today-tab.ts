import { useIsFocused } from 'expo-router';
import * as Linking from 'expo-linking';
import { useEffect, useState, useSyncExternalStore } from 'react';
import { Platform } from 'react-native';
import { openedFromNotification } from '@/lib/device/native-push';
import { storage } from '@/lib/device/storage';
import { plainStart } from '@/lib/device/launch-params';
import { useRooms } from '@/lib/api/rooms';
import { useFeatures } from '@/lib/api/features';

const TAB_KEY = 'qdn.today';
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

function plainLaunch() {
  if (openedFromNotification()) return false;
  if (Platform.OS === 'web') return plainStart();
  const url = Linking.getLinkingURL();
  if (!url) return true;
  const { path, queryParams } = Linking.parse(url);
  return !path?.replace(/^\/+|\/+$/g, '') && !Object.keys(queryParams ?? {}).length;
}

let landed = false;

export function useLandOnToday() {
  const tab = useTodayTab();
  const focused = useIsFocused();
  const [land, setLand] = useState(() => !landed && loaded && signedInAtStart && saved === '1' && plainLaunch());
  useEffect(() => {
    landed = true;
  }, []);
  useEffect(() => {
    if (!land) return;
    const giveUp = setTimeout(() => setLand(false), !focused || !tab ? 0 : 500);
    return () => clearTimeout(giveUp);
  }, [land, focused, tab]);
  return land && focused && tab;
}
