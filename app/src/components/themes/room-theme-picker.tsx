import { Check } from 'lucide-react-native';
import { memo, useState } from 'react';
import { Pressable, Text, View } from 'react-native';

import { SceneArt } from '@/components/themes/scene-art';
import { PressableScale } from '@/components/ui/pressable-scale';
import { useT } from '@/lib/i18n';
import { ACCENTS, useScheme, useTheme } from '@/lib/theme';
import { ROOM_THEMES } from '@/lib/tokens';
import { SCENE_H } from '@/lib/utils/room-art';
import { ACCENT_IDS, CURATED_IDS, type CuratedId, type RoomThemeId } from '@/lib/utils/room-theme';

const GAP = 10;
const POSTER_SCENE_W = 250;
const BORDER = 2;

export function RoomThemePicker({ value, onChange }: { value: RoomThemeId | null; onChange: (id: RoomThemeId | null) => void }) {
  const t = useTheme();
  const tr = useT();
  const pick = (id: RoomThemeId | null) => id !== value && onChange(id);
  const [row, setRow] = useState(0);
  const tile = row ? Math.floor((row - GAP) / 2) : 0;
  return (
    <View style={{ gap: 14 }}>
      <Text style={{ fontFamily: t.font.regular, fontSize: 14, lineHeight: 19, color: t.mut }}>{tr('themes.hint')}</Text>

      <View
        accessibilityRole="radiogroup"
        accessibilityLabel={tr('themes.curated')}
        onLayout={(e) => setRow(Math.floor(e.nativeEvent.layout.width))}
        style={{ flexDirection: 'row', flexWrap: 'wrap', gap: GAP, minHeight: 120 }}>
        {tile > 0 && CURATED_IDS.map((id) => <Poster key={id} id={id} width={tile} on={id === value} onPress={() => pick(id)} />)}
      </View>

      <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>{tr('themes.colours')}</Text>
      <View accessibilityRole="radiogroup" accessibilityLabel={tr('themes.colours')} style={{ flexDirection: 'row', justifyContent: 'space-between' }}>
        {ACCENT_IDS.map((k) => {
          const on = k === value;
          return (
            <Pressable
              key={k}
              accessibilityRole="radio"
              accessibilityLabel={tr('themes.pickA11y', { name: tr(`profile.accents.${k}`) })}
              accessibilityState={{ selected: on }}
              onPress={() => pick(k)}
              hitSlop={4}
              style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', borderWidth: BORDER, borderColor: on ? t.txt : 'transparent' }}>
              <View style={{ width: 28, height: 28, borderRadius: 14, backgroundColor: ACCENTS[k].acc }} />
            </Pressable>
          );
        })}
      </View>

      <PressableScale
        scale={0.98}
        accessibilityRole="radio"
        accessibilityLabel={`${tr('themes.none')}, ${tr('themes.noneHint')}`}
        accessibilityState={{ selected: value == null }}
        onPress={() => pick(null)}
        style={{
          minHeight: 44,
          flexDirection: 'row',
          alignItems: 'center',
          gap: 8,
          paddingHorizontal: 14,
          borderRadius: 14,
          backgroundColor: value == null ? t.accSoft : t.card2,
        }}>
        <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: value == null ? t.accInk : t.txt }}>{tr('themes.none')}</Text>
        <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>
          {tr('themes.noneHint')}
        </Text>
        {value == null && <Check size={18} color={t.accInk} />}
      </PressableScale>
    </View>
  );
}

const Poster = memo(function Poster({ id, width, on, onPress }: { id: CuratedId; width: number; on: boolean; onPress: () => void }) {
  const t = useTheme();
  const tr = useT();
  const scheme = useScheme();
  const c = ROOM_THEMES[id][scheme];
  const name = tr(`themes.names.${id}`);
  const line = tr(`themes.taglines.${id}`);
  const inner = width - 2 * BORDER;
  const sceneH = Math.round((inner * SCENE_H) / POSTER_SCENE_W);
  return (
    <PressableScale
      accessibilityRole="radio"
      accessibilityLabel={`${tr('themes.pickA11y', { name })}, ${line}`}
      accessibilityState={{ selected: on }}
      onPress={onPress}
      style={{ width, borderRadius: 18, overflow: 'hidden', backgroundColor: c.bg, borderWidth: BORDER, borderColor: on ? c.accInk : t.line }}>
      <View pointerEvents="none" style={{ height: sceneH }}>
        <SceneArt id={id} dark={scheme === 'dark'} width={inner} height={sceneH} />
      </View>
      <View style={{ paddingHorizontal: 10, paddingTop: 2, paddingBottom: 10, gap: 6 }}>
        <View>
          <Text numberOfLines={1} style={{ fontFamily: t.font.display, fontSize: 16, letterSpacing: -0.3, color: c.txt }}>
            {name}
          </Text>
          <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 12, lineHeight: 16, color: c.mut }}>
            {line}
          </Text>
        </View>
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 5 }}>
          <View style={{ height: 12, flex: 1, borderRadius: 6, backgroundColor: c.acc }} />
          <View style={{ height: 12, width: 26, borderRadius: 6, backgroundColor: c.tagBg }} />
          <View style={{ height: 12, width: 12, borderRadius: 6, backgroundColor: c.card2 }} />
        </View>
      </View>
      {on && (
        <View
          style={{
            position: 'absolute',
            top: 7,
            left: 7,
            width: 22,
            height: 22,
            borderRadius: 11,
            backgroundColor: c.acc,
            alignItems: 'center',
            justifyContent: 'center',
            borderWidth: BORDER,
            borderColor: c.bg,
          }}>
          <Check size={13} color={c.accTxt} strokeWidth={3.5} />
        </View>
      )}
    </PressableScale>
  );
});
