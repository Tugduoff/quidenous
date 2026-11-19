import { Flame, Star } from 'lucide-react-native';
import { type ComponentRef, useRef } from 'react';
import { TextInput, View } from 'react-native';
import type { Lang } from '@/lib/i18n';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { Card } from '@/components/ui/card';
import { Toggle } from '@/components/ui/toggle';
import { Segmented } from '@/components/ui/segmented';

export function PackMetaCard({
  name,
  onName,
  desc,
  onDesc,
  spicy,
  spicyLocked,
  onSpicy,
  language,
  onLang,
  showFeatured,
  featured,
  onFeatured,
}: {
  name: string;
  onName: (v: string) => void;
  desc: string;
  onDesc: (v: string) => void;
  spicy: boolean;
  spicyLocked: boolean;
  onSpicy: (v: boolean) => void;
  language: Lang;
  onLang: (v: Lang) => void;
  showFeatured: boolean;
  featured: boolean;
  onFeatured: (v: boolean) => void;
}) {
  const t = useTheme();
  const tr = useT();
  const descField = useRef<ComponentRef<typeof TextInput>>(null);
  const input = {
    height: 44,
    paddingHorizontal: 14,
    borderRadius: 14,
    backgroundColor: t.card2,
    fontFamily: t.font.regular,
    fontSize: 16,
    color: t.txt,
  };
  return (
  <Card style={{ padding: 16, gap: 10 }}>
    <TextInput
      accessibilityLabel={tr('packs.name')}
      placeholder={tr('packs.name')}
      placeholderTextColor={t.mut}
      value={name}
      onChangeText={onName}
      maxLength={40}
      returnKeyType="next"
      submitBehavior="submit"
      onSubmitEditing={() => descField.current?.focus()}
      style={input}
    />
    <TextInput
      ref={descField}
      accessibilityLabel={tr('packs.description')}
      placeholder={tr('packs.description')}
      placeholderTextColor={t.mut}
      value={desc}
      onChangeText={onDesc}
      maxLength={140}
      multiline
      style={[input, { height: undefined, minHeight: 44, paddingTop: 11, paddingBottom: 11, textAlignVertical: 'top' }]}
    />
    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 12, marginTop: 4 }}>
      <Toggle label={tr('packs.spicy')} icon={Flame} value={spicy} onChange={(v) => !spicyLocked && onSpicy(v)} />
      <View style={{ flex: 1 }} />
      <Segmented<Lang>
        label={tr('packs.language')}
        options={[
          { id: 'fr', name: 'Français' },
          { id: 'en', name: 'English' },
        ]}
        value={language}
        onChange={onLang}
      />
    </View>
    {showFeatured && <Toggle label={tr('packs.featured')} icon={Star} value={featured} onChange={onFeatured} />}
  </Card>
  );
}
