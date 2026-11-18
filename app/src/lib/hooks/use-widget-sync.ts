import { useEffect } from 'react';
import { Platform } from 'react-native';
import { refreshWidgets } from '@/widget/task';
import { useAppStateChange } from '@/lib/hooks/use-app-state';

export function useWidgetSync(stage: string | null, room: number | null) {
  useEffect(() => {
    if (stage) refreshWidgets();
  }, [stage, room]);
  useAppStateChange((s) => {
    if (s === 'background') refreshWidgets();
  }, Platform.OS === 'android');
}
