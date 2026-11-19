import { Snowflake } from 'lucide-react-native';
import { Text, View } from 'react-native';
import { Card } from '@/components/ui/card';
import { Toggle } from '@/components/ui/toggle';
import { setSeasonalPref, useSeasonalPref } from '@/lib/device/prefs';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';

export function SeasonalRow() {
  const t = useTheme();
  const tr = useT();
  const on = useSeasonalPref();
  return (
    <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 16 }}>
      <Snowflake size={20} color={t.txt} />
      <View style={{ flex: 1, gap: 2 }}>
        <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{tr('season.toggle')}</Text>
        <Text style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>{tr('season.toggleHint')}</Text>
      </View>
      <Toggle label="" a11y={tr('season.toggle')} value={on} onChange={setSeasonalPref} />
    </Card>
  );
}
