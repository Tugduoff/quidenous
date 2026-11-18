import { useLocalSearchParams } from 'expo-router';
import { type ReactNode, useState } from 'react';
import { GestureHandlerRootView } from 'react-native-gesture-handler';
import { SafeAreaView } from 'react-native-safe-area-context';
import { QueryState } from '@/components/ui/query-state';
import { dateFormat, useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { useRoomWrappedOk, useWrapped, type WrappedScope } from '@/lib/api/wrapped';
import { useFeatures } from '@/lib/api/features';
import { Stories } from '@/components/wrapped/stories';
import { WrappedChrome } from '@/components/wrapped/wrapped-chrome';
import { WrappedMessage } from '@/components/wrapped/wrapped-message';

/** "1 décembre" from "2026-12-01". */
function day(iso: string) {
  const [y, m, d] = iso.split('-').map(Number);
  if (!y || !m || !d) return iso;
  return dateFormat({ day: 'numeric', month: 'long' }).format(new Date(y, m - 1, d));
}

/** « Ton année qui de nous » (or the room's): full-screen story cards. `?preview=1`: an admin looking early; `?scope=room`. */
export default function WrappedScreen() {
  const t = useTheme();
  const tr = useT();
  const params = useLocalSearchParams<{ preview?: string; scope?: string }>();
  const preview = params.preview === '1';
  const features = useFeatures();
  const roomOk = useRoomWrappedOk();
  const [chosen, setScope] = useState<WrappedScope>(params.scope === 'room' ? 'room' : 'me');
  // No room (or a party): only mine.
  const scope = roomOk ? chosen : 'me';
  const query = useWrapped(scope, preview);
  const { data: w } = query;

  const chrome = (ink: string) => <WrappedChrome ink={ink} roomOk={roomOk} scope={scope} onScope={setScope} preview={w?.preview} />;

  const showing = !!w && (w.available || w.preview) && w.cards.length > 0;
  if (showing)
    return (
      <GestureHandlerRootView style={{ flex: 1, backgroundColor: t.bg }}>
        <Stories key={`${w.scope}-${w.year}`} w={w} chrome={chrome} />
      </GestureHandlerRootView>
    );

  let body: ReactNode;
  if (features.loaded && !features.wrapped) body = <WrappedMessage who="roupi" mood="sleepy" title={tr('wrapped.unavailable')} line={tr('wrapped.noServer')} />;
  else if (!w) body = <QueryState query={query} style={{ flex: 1, justifyContent: 'center' }} />;
  else if (!w.available && !w.preview)
    body = <WrappedMessage who="roupi" mood="sleepy" title={tr('wrapped.unavailable')} line={tr('wrapped.unavailableLine', { date: day(w.from) })} />;
  else
    body = <WrappedMessage who="pave" mood="think" title={tr(scope === 'room' ? 'wrapped.emptyRoom' : 'wrapped.empty')} line={tr('wrapped.emptyLine')} />;
  return (
    <SafeAreaView edges={['top', 'bottom']} style={{ flex: 1, backgroundColor: t.bg }}>
      {chrome(t.txt)}
      {body}
    </SafeAreaView>
  );
}
