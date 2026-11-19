import { EyeOff } from 'lucide-react-native';
import { Text, TextInput, View } from 'react-native';
import { Card } from '@/components/ui/card';
import { sendOnEnter } from '@/components/ui/send-on-enter';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { ANSWER_MAX, ANSWER_WARN } from '@/lib/utils/said';

export function AnswerField({
  value,
  onChange,
  onSubmit,
  autoFocus,
  error,
}: {
  value: string;
  onChange: (text: string) => void;
  onSubmit: () => void;
  autoFocus?: boolean;
  error?: string | null;
}) {
  const t = useTheme();
  const tr = useT();
  const left = ANSWER_MAX - [...value].length;
  const near = ANSWER_MAX - left >= ANSWER_WARN;
  return (
    <Card style={{ paddingTop: 4, paddingBottom: 12 }}>
      <TextInput
        accessibilityLabel={tr('said.write.label')}
        placeholder={tr('said.write.placeholder')}
        placeholderTextColor={t.mut}
        value={value}
        onChangeText={onChange}
        autoFocus={autoFocus}
        multiline
        maxLength={ANSWER_MAX}
        {...sendOnEnter(onSubmit)}
        style={{
          minHeight: 92,
          paddingHorizontal: 16,
          paddingTop: 12,
          paddingBottom: 6,
          fontFamily: t.font.semibold,
          fontSize: 18,
          lineHeight: 24,
          color: t.txt,
          textAlignVertical: 'top',
        }}
      />
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6, paddingHorizontal: 16 }}>
        <EyeOff size={13} color={t.mut} />
        <Text style={{ flex: 1, fontFamily: t.font.regular, fontSize: 12, lineHeight: 16, color: t.mut }}>{tr('said.write.hint')}</Text>
        {near && (
          <Text
            accessibilityLiveRegion="polite"
            accessibilityLabel={tr('said.write.left', { count: left })}
            style={{ fontFamily: t.font.bold, fontSize: 13, color: left <= 10 ? t.tagInk : t.mut, fontVariant: ['tabular-nums'] }}>
            {left}
          </Text>
        )}
      </View>
      {!!error && (
        <Text accessibilityRole="alert" style={{ paddingHorizontal: 16, paddingTop: 8, fontFamily: t.font.semibold, fontSize: 13, color: t.tagInk }}>
          {error}
        </Text>
      )}
    </Card>
  );
}
