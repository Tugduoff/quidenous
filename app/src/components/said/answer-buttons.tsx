import { Text, View } from 'react-native';
import { Button } from '@/components/ui/button';
import { PressableScale } from '@/components/ui/pressable-scale';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';

export function AnswerButtons({
  editing,
  ok,
  busy,
  onSubmit,
  onCancel,
}: {
  editing: boolean;
  ok: boolean;
  busy: boolean;
  onSubmit: () => void;
  onCancel: () => void;
}) {
  const t = useTheme();
  const tr = useT();
  if (!editing)
    return (
      <Button disabled={!ok} loading={busy} onPress={onSubmit}>
        {tr('said.write.send')}
      </Button>
    );
  return (
    <View style={{ flexDirection: 'row', gap: 10 }}>
      <PressableScale
        accessibilityRole="button"
        onPress={onCancel}
        style={{
          height: 58,
          paddingHorizontal: 22,
          borderRadius: t.radius.pill,
          alignItems: 'center',
          justifyContent: 'center',
          backgroundColor: t.card2,
        }}>
        <Text style={{ fontFamily: t.font.semibold, fontSize: 16, color: t.txt }}>{tr('said.write.cancel')}</Text>
      </PressableScale>
      <Button style={{ flex: 1 }} disabled={!ok} loading={busy} onPress={onSubmit}>
        {tr('said.write.save')}
      </Button>
    </View>
  );
}
