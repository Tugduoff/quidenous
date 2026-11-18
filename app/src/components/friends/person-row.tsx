import type { ReactNode } from 'react';
import { Text, View } from 'react-native';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Avatar } from '@/components/ui/avatar';
import { Card } from '@/components/ui/card';
import type { Member } from '@/lib/api/group';
import type { Person } from '@/lib/api/friends';

export function FriendsLabel({ children }: { children: string }) {
  const t = useTheme();
  return <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4, marginTop: 4 }}>{children}</Text>;
}

export function PersonRow({ person, sub, children }: { person: Person; sub?: string; children?: ReactNode }) {
  const t = useTheme();
  return (
    <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 12 }}>
      <Avatar member={{ id: person.user_id, name: person.name, color: person.color, photo_url: person.photo_url, avatar: person.avatar } as Member} size={40} />
      <View style={{ flex: 1, gap: 2 }}>
        <Text numberOfLines={1} style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>
          {person.name}
        </Text>
        {!!sub && (
          <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 12, color: t.mut }}>
            {sub}
          </Text>
        )}
      </View>
      {children}
    </Card>
  );
}

export function RoundAction({ label, onPress, disabled, tone, children }: { label: string; onPress: () => void; disabled?: boolean; tone?: 'acc'; children: ReactNode }) {
  const t = useTheme();
  return (
    <PressableScale
      accessibilityRole="button"
      accessibilityLabel={label}
      disabled={disabled}
      onPress={onPress}
      hitSlop={6}
      style={{ width: 36, height: 36, borderRadius: 18, alignItems: 'center', justifyContent: 'center', backgroundColor: tone === 'acc' ? t.acc : t.card2 }}>
      {children}
    </PressableScale>
  );
}
