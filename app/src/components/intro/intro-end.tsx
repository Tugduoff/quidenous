import { Text, View } from 'react-native';
import { Mascot } from '@/components/mascot/mascot';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { Display } from '@/components/ui/text';

/** On to the real thing: Zeste, the instigator, sends you off. */
export function IntroEnd({ roomless }: { roomless: boolean }) {
  const t = useTheme();
  const tr = useT();
  return (
    <View style={{ flex: 1, justifyContent: 'center', alignItems: 'center', gap: 16 }}>
      <Mascot who="zeste" mood="celebrate" size={160} />
      <View style={{ gap: 10, alignSelf: 'stretch' }}>
        <Display size={34}>{tr(roomless ? 'intro.end.title' : 'intro.end.inTitle')}</Display>
        <Text style={{ fontFamily: t.font.regular, fontSize: 17, lineHeight: 24, color: t.mut }}>{tr(roomless ? 'intro.end.body' : 'intro.end.inBody')}</Text>
      </View>
    </View>
  );
}
