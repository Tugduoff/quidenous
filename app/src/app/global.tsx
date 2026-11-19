import { Redirect } from 'expo-router';
import { ActivityIndicator, View } from 'react-native';
import { GlobalBallot } from '@/components/global/global-ballot';
import { GlobalResults } from '@/components/global/global-results';
import { QueryState } from '@/components/ui/query-state';
import { Screen } from '@/components/ui/screen';
import { useGlobal } from '@/lib/api/global';
import { useTheme } from '@/lib/theme';
import { BackButton } from '@/components/ui/back-button';

export default function GlobalScreen() {
  const t = useTheme();
  const global = useGlobal();
  const view = global.data;
  const top = (
    <View style={{ flexDirection: 'row', alignItems: 'center' }}>
      <BackButton />
    </View>
  );
  if (!view) {
    return (
      <Screen tabs={false}>
        {top}
        {global.isError ? <QueryState query={global} /> : <ActivityIndicator color={t.mut} style={{ marginTop: 24 }} />}
      </Screen>
    );
  }
  if (!view.available) return <Redirect href="/" />;
  return view.my_vote == null ? <GlobalBallot view={view} top={top} /> : <GlobalResults view={view} top={top} />;
}
