import { Text, View } from 'react-native';
import { MascotBand } from '@/components/mascot/band';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { Display, Logo } from '@/components/ui/text';

/** The whole band waves hello. */
export function IntroHello() {
  const t = useTheme();
  const tr = useT();
  return (
    <View style={{ flex: 1, justifyContent: 'center', gap: 24 }}>
      <Logo size={30} />
      <View style={{ gap: 12 }}>
        <Display size={34}>{tr('intro.hello.title')}</Display>
        <Text style={{ fontFamily: t.font.regular, fontSize: 17, lineHeight: 24, color: t.mut }}>{tr('intro.hello.body')}</Text>
      </View>
      <MascotBand hello max={460} style={{ marginTop: 8 }} />
    </View>
  );
}
