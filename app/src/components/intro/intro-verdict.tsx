import { useMemo } from 'react';
import { Text, View } from 'react-native';
import { DemoBar } from '@/components/intro/demo-bar';
import { Mascot } from '@/components/mascot/mascot';
import { useT } from '@/lib/i18n';
import { demoTally } from '@/lib/utils/intro';
import { type Who } from '@/lib/utils/mascot';
import { useTheme } from '@/lib/theme';
import { Display, QuestionText } from '@/components/ui/text';

export function IntroVerdict({ pick }: { pick: Who }) {
  const t = useTheme();
  const tr = useT();
  const rows = useMemo(() => demoTally(pick), [pick]);
  const right = rows[0]?.who === pick;
  return (
    <>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
        <View style={{ flex: 1, gap: 8 }}>
          <Text style={{ fontFamily: t.font.bold, fontSize: 12, letterSpacing: 0.6, color: t.accInk, textTransform: 'uppercase' }}>
            {tr('intro.verdict.eyebrow')}
          </Text>
          <Display size={28}>{tr(right ? 'intro.verdict.right' : 'intro.verdict.wrong')}</Display>
        </View>
        <Mascot who="bulle" mood="caught" size={104} />
      </View>
      <QuestionText text={tr('intro.vote.question')} rank={false} style={{ fontFamily: t.font.semibold, fontSize: 15, lineHeight: 21, color: t.mut }} />
      <View style={{ gap: 10 }}>
        {rows.map((r, i) => (
          <DemoBar key={r.who} row={r} index={i} rows={rows.length} />
        ))}
      </View>
      <Text style={{ fontFamily: t.font.regular, fontSize: 15, lineHeight: 21, color: t.mut }}>{tr('intro.verdict.body')}</Text>
    </>
  );
}
