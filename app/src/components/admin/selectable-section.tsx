import type { ReactNode } from 'react';
import { Pressable, Text, View } from 'react-native';
import { type Selection, SelectButton, SelectionHeader } from '@/components/ui/selection';
import { useTheme } from '@/lib/theme';

/** A section's title and its "Sélectionner"; while selecting in it, the count, all / none and leave. */
export function SectionTitle<K>({ title, sel, onStart }: { title: string; sel: Selection<K>; onStart: () => void }) {
  const t = useTheme();
  if (sel.on) return <SelectionHeader sel={sel} />;
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8, marginTop: 4 }}>
      <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4 }}>{title}</Text>
      <SelectButton size={32} onPress={() => onStart()} />
    </View>
  );
}

/** A hold on a card (outside its buttons, which keep their own taps) starts selecting. */
export function Hold({ onHold, children }: { onHold: () => void; children: ReactNode }) {
  return (
    <Pressable accessible={false} onLongPress={onHold} delayLongPress={400}>
      {children}
    </Pressable>
  );
}
