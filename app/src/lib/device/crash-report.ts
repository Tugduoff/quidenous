import { Platform } from 'react-native';

import { ApiError } from '@/lib/api/client';
import { appVersion } from '@/lib/device/update';
import { postClientError } from '@/lib/api/client-errors';

const MAX_REPORTS = 5;
const FATAL_WAIT_MS = 2000;

const sent = new Set<string>();

const route = () => (Platform.OS === 'web' && typeof location !== 'undefined' ? location.pathname : undefined);

const messageOf = (error: unknown): string => {
  if (error instanceof Error) return String(error.message || error);
  if (typeof error === 'string') return error;
  try {
    return JSON.stringify(error) ?? String(error);
  } catch {
    return String(error);
  }
};

function isNoise(error: unknown): boolean {
  if (error instanceof ApiError) return true;
  const message = messageOf(error);
  const name = error instanceof Error ? error.name : '';
  if (name === 'AbortError' || name === 'TimeoutError') return true;
  return (
    /^(Network request failed|Failed to fetch|NetworkError when attempting to fetch resource|Load failed|Aborted)\.?$/i.test(message.trim()) ||
    /ResizeObserver loop/.test(message) ||
    /^Script error\.?$/.test(message.trim())
  );
}

export async function reportClientError(error: unknown, extra?: { componentStack?: string; prefix?: string }): Promise<void> {
  try {
    const message = ((extra?.prefix ?? '') + messageOf(error)).slice(0, 500);
    if (!message.trim() || sent.has(message) || sent.size >= MAX_REPORTS) return;
    sent.add(message);
    const stack = error instanceof Error && typeof error.stack === 'string' ? error.stack.slice(0, 4000) : undefined;
    await postClientError({
      message,
      stack,
      componentStack: extra?.componentStack?.slice(0, 4000),
      platform: Platform.OS,
      version: appVersion(),
      route: route(),
    });
  } catch {}
}

type Tracker = {
  hasPromise?: () => boolean;
  enablePromiseRejectionTracker?: (options: {
    allRejections?: boolean;
    onUnhandled?: (id: number, rejection?: unknown) => void;
    onHandled?: (id: number) => void;
  }) => void;
};

let installed = false;

const reportUnexpected = (error: unknown, prefix?: string) =>
  isNoise(error) ? Promise.resolve() : reportClientError(error, prefix ? { prefix } : undefined);

export function installGlobalErrorHandlers() {
  if (installed || __DEV__) return;
  installed = true;
  try {
    if (Platform.OS === 'web') {
      if (typeof window === 'undefined') return;
      window.addEventListener('error', (e) => void reportUnexpected(e.error ?? e.message));
      window.addEventListener('unhandledrejection', (e) => void reportUnexpected(e.reason, 'Unhandled rejection: '));
      return;
    }
    if (typeof ErrorUtils !== 'undefined' && ErrorUtils.getGlobalHandler) {
      const previous = ErrorUtils.getGlobalHandler();
      ErrorUtils.setGlobalHandler((error, isFatal) => {
        const report = reportUnexpected(error, isFatal ? 'Fatal: ' : undefined);
        if (!isFatal) return previous(error, isFatal);
        let done = false;
        const forward = () => {
          if (done) return;
          done = true;
          previous(error, isFatal);
        };
        report.then(forward);
        setTimeout(forward, FATAL_WAIT_MS);
      });
    }
    const hermes = (globalThis as { HermesInternal?: Tracker | null }).HermesInternal;
    if (hermes?.hasPromise?.() && hermes.enablePromiseRejectionTracker) {
      hermes.enablePromiseRejectionTracker({
        allRejections: true,
        onUnhandled: (_id, rejection) => void reportUnexpected(rejection, 'Unhandled rejection: '),
        onHandled: () => {},
      });
    }
  } catch {}
}
