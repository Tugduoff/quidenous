import { Text, View } from 'react-native';
import { Chip } from '@/components/packs/pack-card';
import { useT } from '@/lib/i18n';
import { MAX_TAGS, PACK_TAGS, type PackTag } from '@/lib/api/packs';
import { type Theme, useTheme } from '@/lib/theme';
import { TAG_ICONS } from '@/components/packs/tag-icons';

export const packFormLabel = (t: Theme) => ({ fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4 });

export function PackTagPicker({ tags, onToggle }: { tags: PackTag[]; onToggle: (tag: PackTag) => void }) {
  const t = useTheme();
  const tr = useT();
  const label = packFormLabel(t);
  return (
    <>
      <Text style={label}>{tr('packs.categories')}</Text>
      <View style={{ flexDirection: 'row', flexWrap: 'wrap', gap: 8, marginTop: -6 }}>
        {PACK_TAGS.map((tag) => {
          const on = tags.includes(tag);
          return (
            <Chip
              key={tag}
              icon={TAG_ICONS[tag]}
              label={tr(`packs.tags.${tag}`)}
              on={on}
              disabled={!on && tags.length >= MAX_TAGS}
              onPress={() => onToggle(tag)}
            />
          );
        })}
      </View>
    </>
  );
}
