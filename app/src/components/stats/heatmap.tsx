import { Flame } from 'lucide-react-native';
import { useState } from 'react';
import { Text, View } from 'react-native';
import Svg, { Rect } from 'react-native-svg';

import { Card } from '@/components/ui';
import { useTheme } from '@/lib/theme';

const DAYS = ['Lun', 'Mar', 'Mer', 'Jeu', 'Ven', 'Sam', 'Dim'];
const DAY_NAMES = ['lundi', 'mardi', 'mercredi', 'jeudi', 'vendredi', 'samedi', 'dimanche'];
const LABEL = 32;
const GAP = 2;
const ROW = 14;

/** Votes by weekday (Monday first) and local hour: the busier, the stronger the accent. */
export function Heatmap({ data }: { data: number[][] }) {
  const t = useTheme();
  const [width, setWidth] = useState(0);
  let max = 0;
  let peak = { day: 0, hour: 0 };
  data.forEach((row, d) =>
    row.forEach((v, h) => {
      if (v > max) {
        max = v;
        peak = { day: d, hour: h };
      }
    }),
  );
  const grid = Math.max(0, width - LABEL);
  const cell = (grid - GAP * 23) / 24;
  const x = (h: number) => h * (cell + GAP);
  return (
    <Card style={{ padding: 14, gap: 12 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
        <Flame size={14} color={t.accInk} />
        <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.txt }}>
          Pic : le {DAY_NAMES[peak.day]} vers {peak.hour} h
        </Text>
      </View>
      <View onLayout={(e) => setWidth(e.nativeEvent.layout.width)}>
        {width > 0 && (
          <>
            <View style={{ flexDirection: 'row' }}>
              <View style={{ width: LABEL, gap: GAP }}>
                {DAYS.map((d) => (
                  <Text key={d} style={{ height: ROW, lineHeight: ROW, fontFamily: t.font.regular, fontSize: 10, color: t.mut }}>
                    {d}
                  </Text>
                ))}
              </View>
              <Svg width={grid} height={7 * ROW + 6 * GAP}>
                {data.map((row, d) =>
                  row.map((v, h) => (
                    <Rect
                      key={`${d}-${h}`}
                      x={x(h)}
                      y={d * (ROW + GAP)}
                      width={cell}
                      height={ROW}
                      rx={2.5}
                      fill={v ? t.accInk : t.card2}
                      fillOpacity={v ? 0.18 + 0.82 * (v / max) : 1}
                    />
                  )),
                )}
              </Svg>
            </View>
            <View style={{ height: 14, marginLeft: LABEL, marginTop: 4 }}>
              {[0, 6, 12, 18].map((h) => (
                <Text key={h} style={{ position: 'absolute', left: x(h), fontFamily: t.font.regular, fontSize: 10, color: t.mut }}>
                  {h} h
                </Text>
              ))}
            </View>
          </>
        )}
      </View>
    </Card>
  );
}
