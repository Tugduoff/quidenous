import { Check } from 'lucide-react-native';
import { Text, View } from 'react-native';
import { SectionTitle } from '@/components/choose/section-title';
import { AvatarStack } from '@/components/ui/avatar';
import { Card } from '@/components/ui/card';
import type { Member } from '@/lib/api/group';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import type { ChoosePerson as Person } from '@/lib/utils/choose';

export function AboutMe({ person: p, items, byId }: { person: Person; items: string[]; byId: Map<number, Member> }) {
  const t = useTheme();
  const tr = useT();
  const color = byId.get(p.member_id)?.color ?? t.mut;
  const rows = items.map((label, k) => ({ label, k, voters: p.guesses.filter((g) => g.item === k).map((g) => byId.get(g.voter_id)) }));
  return (
    <View style={{ gap: 10 }}>
      <SectionTitle>{tr('results.choose.aboutMe')}</SectionTitle>
      <Card style={{ padding: 12, gap: 8 }}>
        {rows
          .filter((r) => r.voters.length > 0 || r.k === p.real)
          .map((r) => {
            const real = r.k === p.real;
            return (
              <View
                key={r.k}
                style={{
                  flexDirection: 'row',
                  alignItems: 'center',
                  gap: 10,
                  minHeight: 44,
                  paddingHorizontal: 10,
                  borderRadius: 14,
                  borderWidth: 2,
                  borderColor: real ? color : 'transparent',
                  backgroundColor: t.card2,
                }}>
                <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>
                  {r.label}
                </Text>
                {r.voters.length > 0 ? (
                  <AvatarStack members={r.voters} size={24} ring={t.card2} max={5} />
                ) : (
                  <Text style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>{tr('results.choose.nobody')}</Text>
                )}
                {real && <Check size={16} color={color} strokeWidth={3} />}
              </View>
            );
          })}
      </Card>
    </View>
  );
}
