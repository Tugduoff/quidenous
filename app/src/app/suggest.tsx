import { router } from 'expo-router';
import { Screen } from '@/components/ui/screen';
import { useMySuggestions } from '@/lib/api/community';
import { useT } from '@/lib/i18n';
import { Display } from '@/components/ui/text';
import { BackButton } from '@/components/ui/back-button';
import { SuggestCompose } from '@/components/community/suggest-compose';
import { VoteStack } from '@/components/community/vote-stack';
import { MySuggestions } from '@/components/community/my-suggestions';

export default function SuggestScreen() {
  const tr = useT();
  const mine = useMySuggestions();
  return (
    <Screen tabs={false}>
      <BackButton onPress={() => (router.canGoBack() ? router.back() : router.replace('/questions'))} />
      <Display size={30}>{tr('community.title')}</Display>
      <SuggestCompose left={mine.data?.left} />
      <VoteStack />
      <MySuggestions items={mine.data?.items ?? []} />
    </Screen>
  );
}
