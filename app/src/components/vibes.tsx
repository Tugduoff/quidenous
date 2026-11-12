import { Beer, Briefcase, Coffee, Flame, Heart, House, type LucideIcon, PartyPopper, Sparkles, Users, Wine } from 'lucide-react-native';
import { Text, View } from 'react-native';

import { PressableScale } from '@/components/ui';
import type { Vibe } from '@/lib/api';
import { useTheme } from '@/lib/theme';

/** The server names icons in lucide's kebab-case; one this version doesn't know gets sparkles. */
const ICONS: Record<string, LucideIcon> = {
  beer: Beer,
  flame: Flame,
  briefcase: Briefcase,
  house: House,
  home: House,
  heart: Heart,
  'party-popper': PartyPopper,
  users: Users,
  coffee: Coffee,
  wine: Wine,
  sparkles: Sparkles,
};

export const vibeIcon = (name: string | undefined): LucideIcon => (name && ICONS[name]) || Sparkles;

/** The tones as selectable cards (icon, name, one line). */
export function VibeCards({ vibes, value, onChange }: { vibes: Vibe[]; value: string | null; onChange: (id: string) => void }) {
  const t = useTheme();
  return (
    <View style={{ gap: 10 }}>
      {vibes.map((v) => {
        const on = v.id === value;
        const Icon = vibeIcon(v.icon);
        return (
          <PressableScale
            key={v.id}
            scale={0.98}
            accessibilityRole="radio"
            accessibilityState={{ selected: on }}
            accessibilityLabel={v.name}
            onPress={() => onChange(v.id)}
            style={[
              {
                flexDirection: 'row',
                alignItems: 'center',
                gap: 14,
                padding: 14,
                borderRadius: 20,
                borderWidth: 2,
                borderColor: on ? t.accInk : 'transparent',
                backgroundColor: on ? t.accSoft : t.card,
              },
              t.shadow,
            ]}>
            <View
              style={{
                width: 44,
                height: 44,
                borderRadius: 22,
                alignItems: 'center',
                justifyContent: 'center',
                backgroundColor: on ? t.acc : t.card2,
              }}>
              <Icon size={22} color={on ? t.accTxt : t.txt} />
            </View>
            <View style={{ flex: 1, gap: 2 }}>
              <Text numberOfLines={1} style={{ fontFamily: t.font.bold, fontSize: 16, color: t.txt }}>
                {v.name}
              </Text>
              {!!v.description && (
                <Text numberOfLines={2} style={{ fontFamily: t.font.regular, fontSize: 13, lineHeight: 18, color: t.mut }}>
                  {v.description}
                </Text>
              )}
            </View>
          </PressableScale>
        );
      })}
    </View>
  );
}
