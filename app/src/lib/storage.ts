import * as SecureStore from 'expo-secure-store';
import { Platform } from 'react-native';

const web = Platform.OS === 'web';

export const storage = {
  get: (key: string) =>
    web ? Promise.resolve(localStorage.getItem(key)) : SecureStore.getItemAsync(key),
  set: (key: string, value: string) =>
    web ? Promise.resolve(localStorage.setItem(key, value)) : SecureStore.setItemAsync(key, value),
  remove: (key: string) =>
    web ? Promise.resolve(localStorage.removeItem(key)) : SecureStore.deleteItemAsync(key),
};

/** The session (lib/session.tsx), also read by the home-screen widget's background task (widget/state.ts). */
export const TOKEN_KEY = 'qdn.token';
export const SERVER_KEY = 'qdn.server';
export const ROOM_KEY = 'qdn.room';
