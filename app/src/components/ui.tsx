import type { ReactNode } from 'react';
import { ActivityIndicator, Pressable, StyleSheet, Text, View, type ViewStyle } from 'react-native';
import { Image } from 'expo-image';

import { photoUrl, type Member } from '@/lib/api';
import { useTheme } from '@/lib/theme';

export function Avatar({ member, size = 40 }: { member?: Member; size?: number }) {
  const t = useTheme();
  const uri = member && photoUrl(member);
  return (
    <View
      style={{
        width: size,
        height: size,
        borderRadius: size / 2,
        backgroundColor: member?.color ?? t.card2,
        alignItems: 'center',
        justifyContent: 'center',
        overflow: 'hidden',
      }}>
      {uri ? (
        <Image source={{ uri }} style={{ width: size, height: size }} />
      ) : (
        <Text style={{ fontFamily: t.font.bold, fontSize: size * 0.42, color: '#121216' }}>
          {member?.name[0]?.toUpperCase() ?? '?'}
        </Text>
      )}
    </View>
  );
}

export function AvatarStack({ members, size = 28 }: { members: (Member | undefined)[]; size?: number }) {
  const t = useTheme();
  return (
    <View style={{ flexDirection: 'row' }}>
      {members.map((m, i) => (
        <View
          key={m?.id ?? i}
          style={{ marginLeft: i ? -8 : 0, borderRadius: size, borderWidth: 2, borderColor: t.bg }}>
          <Avatar member={m} size={size} />
        </View>
      ))}
    </View>
  );
}

export function Button({
  children,
  onPress,
  disabled,
  loading,
  style,
}: {
  children: ReactNode;
  onPress?: () => void;
  disabled?: boolean;
  loading?: boolean;
  style?: ViewStyle;
}) {
  const t = useTheme();
  const off = disabled || loading;
  return (
    <Pressable
      accessibilityRole="button"
      disabled={off}
      onPress={onPress}
      style={({ pressed }) => [
        styles.button,
        { backgroundColor: off ? t.card2 : t.acc, opacity: pressed ? 0.85 : 1 },
        style,
      ]}>
      {loading ? (
        <ActivityIndicator color={t.mut} />
      ) : typeof children === 'string' ? (
        <Text style={{ fontFamily: t.font.bold, fontSize: 17, color: off ? t.mut : t.accTxt }}>{children}</Text>
      ) : (
        children
      )}
    </Pressable>
  );
}

export function Tag({ label, tone = 'tag' }: { label: string; tone?: 'tag' | 'acc' }) {
  const t = useTheme();
  return (
    <View style={[styles.tag, { backgroundColor: tone === 'acc' ? t.acc : t.tag }]}>
      <Text
        style={{
          fontFamily: t.font.bold,
          fontSize: 12,
          letterSpacing: 0.3,
          color: tone === 'acc' ? t.accTxt : '#fff',
          textTransform: 'uppercase',
        }}>
        {label}
      </Text>
    </View>
  );
}

export function Card({ children, style }: { children: ReactNode; style?: ViewStyle }) {
  const t = useTheme();
  return <View style={[{ backgroundColor: t.card, borderRadius: t.radius.card }, t.shadow, style]}>{children}</View>;
}

export function Display({ children, size = 34 }: { children: ReactNode; size?: number }) {
  const t = useTheme();
  return (
    <Text style={{ fontFamily: t.font.display, fontSize: size, lineHeight: size * 1.08, letterSpacing: -1, color: t.txt }}>
      {children}
    </Text>
  );
}

export function Logo({ size = 22 }: { size?: number }) {
  const t = useTheme();
  return (
    <Text style={{ fontFamily: t.font.display, fontSize: size, letterSpacing: -0.5, color: t.txt }}>
      qui de nous<Text style={{ color: t.accInk }}>?</Text>
    </Text>
  );
}

const styles = StyleSheet.create({
  button: { height: 58, borderRadius: 999, alignItems: 'center', justifyContent: 'center', flexDirection: 'row', gap: 8 },
  tag: { alignSelf: 'flex-start', borderRadius: 999, paddingHorizontal: 10, paddingVertical: 5 },
});
