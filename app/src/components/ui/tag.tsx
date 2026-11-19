import { useTheme } from '@/lib/theme';
import { StyleSheet, Text, View } from 'react-native';

export function Tag({ label, tone = 'tag' }: { label: string; tone?: 'tag' | 'acc' }) {
  const t = useTheme();
  return (
    <View style={[styles.tag, { backgroundColor: tone === 'acc' ? t.acc : t.tagBg }]}>
      <Text
        style={{
          fontFamily: t.font.bold,
          fontSize: 12,
          letterSpacing: 0.3,
          color: tone === 'acc' ? t.accTxt : t.tagTxt,
          textTransform: 'uppercase',
        }}>
        {label}
      </Text>
    </View>
  );
}

const styles = StyleSheet.create({
  tag: { alignSelf: 'flex-start', borderRadius: 999, paddingHorizontal: 10, paddingVertical: 5 },
});
