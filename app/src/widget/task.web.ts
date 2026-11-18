// The web has no home-screen widget: this stand-in keeps react-native-android-widget, expo-task-manager and
// expo-notifications out of the web bundle (see task.tsx, docs/WIDGET.md).
export const WIDGETS = [] as const;

export function registerWidget() {}

export async function refreshWidgets() {}

export async function forgetWidget() {}

export function useWidgetSync(_stage: string | null, _room: number | null) {}
