import { router } from 'expo-router';
import { Star } from 'lucide-react-native';
import { Text, View } from 'react-native';
import { MiniTag, StatusBadge } from '@/components/packs/pack-card';
import { useT } from '@/lib/i18n';
import { isPackTag, type PackDetail } from '@/lib/api/packs';
import { useTheme } from '@/lib/theme';
import { Display } from '@/components/ui/text';
import { TAG_ICONS } from '@/components/packs/tag-icons';

export function PackSummary({ pack, listed, roomLanguage }: { pack: PackDetail; listed: boolean; roomLanguage: string | null | undefined }) {
  const t = useTheme();
  const tr = useT();
  const tags = pack.tags.filter(isPackTag);
  const otherLang = !!pack.language && !!roomLanguage && pack.language !== roomLanguage;
  const marks = pack.featured || pack.spicy || otherLang || tags.length > 0 || (pack.mine && !listed);
  return (
  <View style={{ gap: 6 }}>
    <Display size={28}>{pack.name}</Display>
    <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 14, color: t.mut }}>
      {`${tr('packs.questions', { count: pack.questions.length })} · `}
      <Text
        accessibilityRole="link"
        onPress={() => router.push(`/creator/${pack.author_id}`)}
        style={{ fontFamily: t.font.semibold, color: t.accInk }}>
        {pack.author}
      </Text>
    </Text>
    {marks && (
      <View style={{ flexDirection: 'row', flexWrap: 'wrap', alignItems: 'center', gap: 6 }}>
        {pack.mine && <StatusBadge status={pack.status} />}
        {pack.featured && (
          <View
            style={{
              flexDirection: 'row',
              alignItems: 'center',
              gap: 4,
              height: 22,
              paddingHorizontal: 8,
              borderRadius: 11,
              backgroundColor: t.accSoft,
            }}>
            <Star size={12} color={t.accInk} fill={t.accInk} />
            <Text style={{ fontFamily: t.font.bold, fontSize: 11, color: t.accInk }}>{tr('packs.featured')}</Text>
          </View>
        )}
        {pack.spicy && <MiniTag label={tr('packs.spicy')} tone="hot" />}
        {otherLang && pack.language && <MiniTag label={pack.language.toUpperCase()} />}
        {tags.map((tag) => {
          const Icon = TAG_ICONS[tag];
          return (
            <View
              key={tag}
              style={{
                flexDirection: 'row',
                alignItems: 'center',
                gap: 4,
                height: 22,
                paddingHorizontal: 8,
                borderRadius: 11,
                backgroundColor: t.card2,
              }}>
              <Icon size={12} color={t.mut} />
              <Text style={{ fontFamily: t.font.semibold, fontSize: 11, color: t.mut }}>{tr(`packs.tags.${tag}`)}</Text>
            </View>
          );
        })}
      </View>
    )}
    {pack.mine && pack.status === 'removed' && !!pack.removed_reason && (
      <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.tagInk }}>
        {tr('packs.removedReason', { reason: pack.removed_reason })}
      </Text>
    )}
    {!!pack.description && <Text style={{ fontFamily: t.font.regular, fontSize: 15, lineHeight: 21, color: t.txt }}>{pack.description}</Text>}
  </View>
  );
}
