import { Plus, X } from 'lucide-react-native';
import type { Dispatch, SetStateAction } from 'react';
import { Pressable, Text, TextInput, View } from 'react-native';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { ITEM_MAX, ITEMS_MAX, ITEMS_MIN } from '@/lib/utils/import-draft';

/** A choose question's items: 2 to 4, one per line, with what's wrong with them on the right. */
export function ChooseItemsEditor({
  items,
  setItems,
  onSubmit,
  sameItems,
  error,
}: {
  items: string[];
  setItems: Dispatch<SetStateAction<string[]>>;
  onSubmit: () => void;
  sameItems: boolean;
  error: string | null;
}) {
  const t = useTheme();
  const tr = useT();
  const setItem = (i: number, v: string) => setItems((l) => l.map((x, j) => (j === i ? v : x)));
  return (
    <View style={{ paddingHorizontal: 12, paddingBottom: 10, gap: 8 }}>
      {items.map((item, i) => (
        <View key={i} style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
          <TextInput
            accessibilityLabel={tr('questions.item', { n: i + 1 })}
            placeholder={tr('questions.item', { n: i + 1 })}
            placeholderTextColor={t.mut}
            value={item}
            onChangeText={(v) => setItem(i, v)}
            maxLength={ITEM_MAX}
            returnKeyType="send"
            submitBehavior="blurAndSubmit"
            onSubmitEditing={onSubmit}
            style={{
              flex: 1,
              height: 44,
              paddingHorizontal: 12,
              borderRadius: 12,
              backgroundColor: t.card2,
              fontFamily: t.font.regular,
              fontSize: 16,
              color: t.txt,
            }}
          />
          {items.length > ITEMS_MIN && (
            <Pressable
              accessibilityRole="button"
              accessibilityLabel={tr('questions.removeItem', { n: i + 1 })}
              onPress={() => setItems((l) => l.filter((_, j) => j !== i))}
              hitSlop={6}
              style={{ width: 34, height: 34, borderRadius: 17, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card2 }}>
              <X size={16} color={t.mut} />
            </Pressable>
          )}
        </View>
      ))}
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10, minHeight: 32 }}>
        {items.length < ITEMS_MAX && (
          <Pressable
            accessibilityRole="button"
            onPress={() => setItems((l) => [...l, ''])}
            hitSlop={6}
            style={{ flexDirection: 'row', alignItems: 'center', gap: 6, paddingVertical: 6, paddingHorizontal: 4 }}>
            <Plus size={16} color={t.accInk} />
            <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.accInk }}>{tr('questions.addItem')}</Text>
          </Pressable>
        )}
        {(sameItems || error) && (
          <Text numberOfLines={1} style={{ flex: 1, textAlign: 'right', fontFamily: t.font.regular, fontSize: 13, color: t.tagInk }}>
            {sameItems ? tr('questions.sameItems') : error}
          </Text>
        )}
      </View>
    </View>
  );
}
