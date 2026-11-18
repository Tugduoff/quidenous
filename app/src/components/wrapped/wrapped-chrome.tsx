import { X } from 'lucide-react-native';
import { Pressable, Text, View } from 'react-native';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import type { WrappedScope } from '@/lib/api/wrapped';
import { ScopeToggle } from '@/components/wrapped/scope-toggle';
import { closeWrapped, INK } from '@/components/wrapped/constants';

export function WrappedChrome({
  ink,
  roomOk,
  scope,
  onScope,
  preview,
}: {
  ink: string;
  roomOk: boolean;
  scope: WrappedScope;
  onScope: (s: WrappedScope) => void;
  preview: boolean | undefined;
}) {
  const t = useTheme();
  const tr = useT();
  return (
  <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10, paddingHorizontal: 12, paddingTop: 10, minHeight: 50 }}>
    {roomOk && <ScopeToggle value={scope} onChange={onScope} ink={ink} track={ink === INK ? '#1212161F' : t.card2} />}
    <View style={{ flex: 1 }} />
    {preview && (
      <View style={{ paddingHorizontal: 10, height: 24, borderRadius: 12, justifyContent: 'center', borderWidth: 1.5, borderColor: ink }}>
        <Text style={{ fontFamily: t.font.bold, fontSize: 11, letterSpacing: 0.5, textTransform: 'uppercase', color: ink }}>{tr('wrapped.preview')}</Text>
      </View>
    )}
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={tr('wrapped.close')}
      onPress={closeWrapped}
      hitSlop={8}
      style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: ink === INK ? '#1212161F' : t.card }}>
      <X size={20} color={ink} />
    </Pressable>
  </View>
  );
}
