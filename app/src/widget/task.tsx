'use no memo';
// The home-screen widget's background side (docs/WIDGET.md), registered from the entry (index.ts) so it runs with the app
// closed: the launcher's updates (added, every hour, resized), the silent FCM "refresh" push, and refreshes from the app.
import * as Notifications from 'expo-notifications';
import * as TaskManager from 'expo-task-manager';
import { Platform } from 'react-native';
import { getWidgetInfo, registerWidgetTaskHandler, requestWidgetUpdate, type WidgetTaskHandlerProps } from 'react-native-android-widget';

import { deviceLang } from '@/lib/i18n';

import { forgetWidgetToken, loadWidgetView, type WidgetView } from '@/widget/state';
import { renderBoth } from '@/widget/widget';

/** The widget names in app.json (react-native-android-widget's plugin): same drawing, two default sizes. */
export const WIDGETS = ['QdnSmall', 'QdnMedium'] as const;
const PUSH_TASK = 'qdn-widget-refresh';

/** Several widgets (and a push right after the app asked) share one request. */
let inflight: Promise<WidgetView> | null = null;
let last: WidgetView | null = null;
const load = () =>
  (inflight ??= loadWidgetView()
    .then((v) => (last = v))
    .finally(() => {
      inflight = null;
    }));

async function anyWidget() {
  const all = await Promise.all(WIDGETS.map((w) => getWidgetInfo(w).catch(() => [])));
  return all.some((list) => list.length > 0);
}

async function onWidgetTask({ widgetInfo, widgetAction, renderWidget }: WidgetTaskHandlerProps) {
  switch (widgetAction) {
    case 'WIDGET_ADDED':
    case 'WIDGET_UPDATE':
      renderWidget(renderBoth(await load(), widgetInfo.width, widgetInfo.height, deviceLang()));
      break;
    // A new size, same data: no request.
    case 'WIDGET_RESIZED':
      renderWidget(renderBoth(last ?? (await load()), widgetInfo.width, widgetInfo.height, deviceLang()));
      break;
    // The last one gone from the home screen: no more pushes for it, no token left behind.
    case 'WIDGET_DELETED':
      if (!(await anyWidget())) await forgetWidgetToken();
      break;
  }
}

/** Redraws every widget on the home screen with fresh data (nothing to do without one). */
export async function refreshWidgets() {
  if (Platform.OS !== 'android') return;
  try {
    if (!(await anyWidget())) return;
    const view = await load();
    await Promise.all(
      WIDGETS.map((widgetName) =>
        requestWidgetUpdate({ widgetName, renderWidget: (info) => renderBoth(view, info.width, info.height, deviceLang()) }),
      ),
    );
  } catch {}
}

/** Signed out or forgot the session on this phone: the widget forgets it too, and says so. */
export async function forgetWidget() {
  if (Platform.OS !== 'android') return;
  await forgetWidgetToken();
  await refreshWidgets();
}

/** The server's silent push (`widget: "1"`, server/src/push.rs `widget_ping_message`), in either shape Android hands it. */
function isWidgetPing(payload: unknown) {
  const data = (payload as { data?: Record<string, unknown> } | null)?.data;
  if (!data) return false;
  if (data.widget === '1') return true;
  try {
    return typeof data.dataString === 'string' && JSON.parse(data.dataString)?.widget === '1';
  } catch {
    return false;
  }
}

/** Once, from the entry: the widget's task and the push task. Android only (no widget elsewhere yet). */
export function registerWidget() {
  if (Platform.OS !== 'android') return;
  registerWidgetTaskHandler(onWidgetTask);
  TaskManager.defineTask<Notifications.NotificationTaskPayload>(PUSH_TASK, async ({ data }) => {
    if (isWidgetPing(data)) await refreshWidgets();
  });
  Notifications.registerTaskAsync(PUSH_TASK).catch(() => {});
}
