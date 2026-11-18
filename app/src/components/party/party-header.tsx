import { router } from 'expo-router';
import { ChevronLeft, Ellipsis } from 'lucide-react-native';
import { useState } from 'react';
import { Pressable, Text, View } from 'react-native';
import { PartyMenu } from '@/components/party/party-menu';
import { PressableScale } from '@/components/ui/pressable-scale';
import type { Party } from '@/lib/api/party';
import { dateFormat, useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';

export function BackToRooms() {
  const t = useTheme();
  const tr = useT();
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={tr('common.back')}
      onPress={() => router.replace('/rooms')}
      hitSlop={4}
      style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
      <ChevronLeft size={22} color={t.txt} />
    </Pressable>
  );
}

export function PartyHeader({ party }: { party: Party }) {
  const t = useTheme();
  const tr = useT();
  const [menu, setMenu] = useState(false);
  const sub =
    party.phase === 'over'
      ? tr('party.over')
      : tr('party.until', { time: dateFormat({ hour: '2-digit', minute: '2-digit' }).format(new Date(party.ends_at)) });
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 12 }}>
      <BackToRooms />
      <View style={{ flex: 1, minWidth: 0 }}>
        <Text numberOfLines={1} style={{ fontFamily: t.font.bold, fontSize: 17, color: t.txt }}>
          {party.name || tr('party.name')}
        </Text>
        <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>
          {sub}
        </Text>
      </View>
      <PressableScale
        accessibilityRole="button"
        accessibilityLabel={tr('party.menu.a11y')}
        onPress={() => setMenu(true)}
        hitSlop={4}
        style={[{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }, t.shadow]}>
        <Ellipsis size={20} color={t.txt} />
      </PressableScale>
      <PartyMenu party={party} visible={menu} onClose={() => setMenu(false)} />
    </View>
  );
}
