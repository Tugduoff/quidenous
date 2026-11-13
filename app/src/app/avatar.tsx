import { useQueryClient } from '@tanstack/react-query';
import { router } from 'expo-router';
import { ChevronLeft, Dices } from 'lucide-react-native';
import { type ReactNode, useState } from 'react';
import { Pressable, ScrollView, Text, useWindowDimensions, View } from 'react-native';
import Animated from 'react-native-reanimated';

import { AvatarArt } from '@/components/avatar-art';
import { Screen } from '@/components/screen';
import { Button, PressableScale } from '@/components/ui';
import { api, type Member } from '@/lib/api';
import {
  type Avatar,
  type Field,
  formatAvatar,
  optionCount,
  PARTS,
  type PaletteField,
  parseAvatar,
  PRESETS,
  randomAvatar,
  startingAvatar,
} from '@/lib/avatar';
import { usePop } from '@/lib/motion';
import { keys } from '@/lib/queries';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';

type Tab = { label: string; field?: Field; colors?: PaletteField; zoom?: boolean };
const TABS: Tab[] = [
  { label: 'Modèles' },
  { label: 'Visage', field: 'face', colors: 'skin' },
  { label: 'Cheveux', field: 'hair', colors: 'hairColor' },
  { label: 'Yeux', field: 'eyes', zoom: true },
  { label: 'Sourcils', field: 'brows', zoom: true },
  { label: 'Bouche', field: 'mouth', zoom: true },
  { label: 'Lunettes', field: 'glasses', zoom: true },
  { label: 'Barbe', field: 'beard', colors: 'hairColor', zoom: true },
  { label: 'Accessoires', field: 'extra' },
  { label: 'Fond', colors: 'bg' },
];
/** Close-up on the face, for the small features. */
const ZOOM = '20 24 60 60';

function Round({ size, children }: { size: number; children: ReactNode }) {
  return <View style={{ width: size, height: size, borderRadius: size / 2, overflow: 'hidden' }}>{children}</View>;
}

/** One choice: the avatar as it would be with it. */
function Choice({
  avatar,
  on,
  onPress,
  width,
  zoom,
  label,
}: {
  avatar: Avatar;
  on: boolean;
  onPress: () => void;
  width: number;
  zoom?: boolean;
  label: string;
}) {
  const t = useTheme();
  const inner = Math.round(width - 16);
  return (
    <PressableScale
      accessibilityRole="radio"
      accessibilityLabel={label}
      accessibilityState={{ selected: on }}
      onPress={onPress}
      style={[
        { width, height: width, borderRadius: 22, alignItems: 'center', justifyContent: 'center', borderWidth: 2 },
        { backgroundColor: on ? t.accSoft : t.card, borderColor: on ? t.accInk : 'transparent' },
        t.shadow,
      ]}>
      <Round size={inner}>
        <AvatarArt avatar={avatar} size={inner} viewBox={zoom ? ZOOM : undefined} />
      </Round>
    </PressableScale>
  );
}

function Swatches({ field, value, onPick }: { field: PaletteField; value: number; onPick: (i: number) => void }) {
  const t = useTheme();
  const big = field === 'bg';
  const outer = big ? 52 : 44;
  return (
    <View style={{ flexDirection: 'row', flexWrap: 'wrap', gap: big ? 10 : 6 }}>
      {PARTS[field].map(([c], i) => {
        const on = i === value;
        return (
          <Pressable
            key={c}
            accessibilityRole="radio"
            accessibilityLabel={`Couleur ${i + 1}`}
            accessibilityState={{ selected: on }}
            onPress={() => onPick(i)}
            style={{
              width: outer,
              height: outer,
              borderRadius: outer / 2,
              alignItems: 'center',
              justifyContent: 'center',
              borderWidth: 2,
              borderColor: on ? t.txt : 'transparent',
            }}>
            <View style={{ width: outer - 12, height: outer - 12, borderRadius: outer, backgroundColor: c }} />
          </Pressable>
        );
      })}
    </View>
  );
}

/** The avatar builder: presets, every part, a dice; saving replaces the photo. */
export default function AvatarBuilder() {
  const t = useTheme();
  const qc = useQueryClient();
  const { me, setMe } = useSession();
  const { width: screen } = useWindowDimensions();
  const [avatar, setAvatar] = useState<Avatar>(() => startingAvatar(me?.avatar, me?.color));
  const [tab, setTab] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(false);
  const spec = formatAvatar(avatar);
  const pop = usePop(spec, 1.06);

  const content = Math.min(screen, 560) - 40;
  const cols = Math.max(4, Math.floor(content / 100));
  const cell = Math.floor((content - (cols - 1) * 10) / cols);
  const set = (f: Field, i: number) => setAvatar((a) => ({ ...a, [f]: i }));
  const { field, colors, zoom } = TABS[tab];

  const save = async () => {
    setBusy(true);
    setError(false);
    try {
      setMe(await api<Member>('/me/avatar', { method: 'PUT', json: { avatar: spec } }));
      qc.invalidateQueries({ queryKey: keys.group });
      router.back();
    } catch {
      setError(true);
      setBusy(false);
    }
  };

  return (
    <Screen
      tabs={false}
      footer={
        <>
          {error && <Text style={{ fontFamily: t.font.semibold, color: t.tag, textAlign: 'center' }}>Avatar non enregistré</Text>}
          <Button onPress={save} loading={busy} disabled={spec === me?.avatar && !me?.photo_url}>
            Enregistrer
          </Button>
        </>
      }>
      <View style={{ flexDirection: 'row', justifyContent: 'space-between' }}>
        <Pressable
          accessibilityRole="button"
          accessibilityLabel="Retour"
          onPress={() => router.back()}
          style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
          <ChevronLeft size={22} color={t.txt} />
        </Pressable>
        <PressableScale
          accessibilityRole="button"
          accessibilityLabel="Aléatoire"
          onPress={() => setAvatar(randomAvatar())}
          style={{
            flexDirection: 'row',
            alignItems: 'center',
            gap: 8,
            height: 40,
            paddingHorizontal: 16,
            borderRadius: 20,
            backgroundColor: t.card,
          }}>
          <Dices size={18} color={t.txt} />
          <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>Aléatoire</Text>
        </PressableScale>
      </View>

      <Animated.View style={[{ alignSelf: 'center' }, pop]}>
        <Round size={168}>
          <AvatarArt avatar={avatar} size={168} />
        </Round>
      </Animated.View>

      <ScrollView
        horizontal
        showsHorizontalScrollIndicator={false}
        style={{ marginHorizontal: -20, flexGrow: 0 }}
        contentContainerStyle={{ paddingHorizontal: 20, gap: 8 }}>
        {TABS.map((tb, i) => {
          const on = i === tab;
          return (
            <Pressable
              key={tb.label}
              accessibilityRole="tab"
              accessibilityState={{ selected: on }}
              onPress={() => setTab(i)}
              style={{ height: 38, paddingHorizontal: 16, borderRadius: 19, justifyContent: 'center', backgroundColor: on ? t.acc : t.card }}>
              <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: on ? t.accTxt : t.txt }}>{tb.label}</Text>
            </Pressable>
          );
        })}
      </ScrollView>

      {colors && <Swatches field={colors} value={avatar[colors]} onPick={(i) => set(colors, i)} />}

      <View style={{ flexDirection: 'row', flexWrap: 'wrap', gap: 10 }}>
        {!field && !colors
          ? PRESETS.map((p, i) => {
              const a = parseAvatar(p)!;
              return <Choice key={p} avatar={a} on={p === spec} onPress={() => setAvatar(a)} width={cell} label={`Modèle ${i + 1}`} />;
            })
          : field &&
            Array.from({ length: optionCount(field) }, (_, i) => (
              <Choice
                key={i}
                avatar={{ ...avatar, [field]: i }}
                on={avatar[field] === i}
                onPress={() => set(field, i)}
                width={cell}
                zoom={zoom}
                label={`${TABS[tab].label} ${i + 1}`}
              />
            ))}
      </View>
    </Screen>
  );
}
