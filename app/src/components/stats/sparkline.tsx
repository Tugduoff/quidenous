import { useState } from 'react';
import { Text, View } from 'react-native';
import Svg, { Circle, Defs, LinearGradient, Path, Stop } from 'react-native-svg';

import { Card } from '@/components/ui';
import type { Activity } from '@/lib/api';
import { dateFormat, useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';

import { dec, int } from './common';

const HEIGHT = 72;
const PAD = 4;

const parseDay = (s: string) => {
  const [y, m, d] = s.split('-').map(Number);
  return new Date(y, (m || 1) - 1, d || 1);
};

/** Questions per day (or week): the total, the average, and the curve under it. */
export function Sparkline({ activity }: { activity: Activity }) {
  const t = useTheme();
  const tr = useT();
  const [width, setWidth] = useState(0);
  const values = activity.questions;
  const total = values.reduce((a, b) => a + b, 0);
  const max = Math.max(1, ...values);
  const step = values.length > 1 ? (width - PAD * 2) / (values.length - 1) : 0;
  const pts = values.map((v, i) => [PAD + i * step, PAD + (HEIGHT - PAD * 2) * (1 - v / max)] as const);
  const line = pts.map(([x, y], i) => `${i ? 'L' : 'M'}${x.toFixed(1)},${y.toFixed(1)}`).join(' ');
  const area = pts.length ? `${line} L${pts.at(-1)![0].toFixed(1)},${HEIGHT} L${pts[0][0].toFixed(1)},${HEIGHT} Z` : '';
  const last = pts.at(-1);
  const week = activity.bucket === 'week';
  return (
    <Card style={{ padding: 16, gap: 12 }}>
      <View style={{ flexDirection: 'row', alignItems: 'flex-end', justifyContent: 'space-between', gap: 12 }}>
        <View style={{ flexDirection: 'row', alignItems: 'baseline', gap: 6 }}>
          <Text style={{ fontFamily: t.font.display, fontSize: 32, letterSpacing: -1, color: t.txt }}>{int(total)}</Text>
          <Text style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>{tr('stats.unit.questions', { count: total })}</Text>
        </View>
        <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.accInk, paddingBottom: 6 }}>
          {tr(week ? 'stats.rhythm.perWeek' : 'stats.rhythm.perDay', { n: dec(total / values.length) })}
        </Text>
      </View>
      <View onLayout={(e) => setWidth(e.nativeEvent.layout.width)} style={{ height: HEIGHT }}>
        {width > 0 && (
          <Svg width={width} height={HEIGHT}>
            <Defs>
              <LinearGradient id="spark" x1="0" y1="0" x2="0" y2="1">
                <Stop offset="0" stopColor={t.accInk} stopOpacity={0.35} />
                <Stop offset="1" stopColor={t.accInk} stopOpacity={0} />
              </LinearGradient>
            </Defs>
            <Path d={area} fill="url(#spark)" />
            <Path d={line} stroke={t.accInk} strokeWidth={2.5} strokeLinejoin="round" strokeLinecap="round" fill="none" />
            {last && <Circle cx={last[0]} cy={last[1]} r={4} fill={t.accInk} stroke={t.card} strokeWidth={2} />}
          </Svg>
        )}
      </View>
      <View style={{ flexDirection: 'row', justifyContent: 'space-between' }}>
        <Text style={{ fontFamily: t.font.regular, fontSize: 11, color: t.mut }}>
          {dateFormat({ day: 'numeric', month: 'short' }).format(parseDay(activity.start))}
        </Text>
        <Text style={{ fontFamily: t.font.regular, fontSize: 11, color: t.mut }}>{tr(week ? 'stats.rhythm.thisWeek' : 'stats.rhythm.today')}</Text>
      </View>
    </Card>
  );
}
