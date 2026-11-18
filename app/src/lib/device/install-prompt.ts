import { Platform } from 'react-native';

export type InstallPrompt = Event & { prompt: () => Promise<void>; userChoice: Promise<{ outcome: 'accepted' | 'dismissed' }> };

let deferred: InstallPrompt | null = null;
const waiting = new Set<() => void>();
if (Platform.OS === 'web' && typeof window !== 'undefined') {
  window.addEventListener('beforeinstallprompt', (e) => {
    e.preventDefault();
    deferred = e as InstallPrompt;
    waiting.forEach((f) => f());
  });
}

export const installPrompt = () => deferred;

export const forgetInstallPrompt = () => {
  deferred = null;
};

export function onInstallPrompt(ready: () => void): () => void {
  waiting.add(ready);
  return () => void waiting.delete(ready);
}
