import { Tip } from '@/components/ui/tip';
import { useT } from '@/lib/i18n';

export function VoteTip() {
  const tr = useT();
  return <Tip id="vote" text={tr('vote.tip')} />;
}
