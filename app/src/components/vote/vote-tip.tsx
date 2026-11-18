import { Tip } from '@/components/ui/tip';
import { useT } from '@/lib/i18n';

/** First-run hint on the ballot: how a question ends. */
export function VoteTip() {
  const tr = useT();
  return <Tip id="vote" text={tr('vote.tip')} />;
}
