import { Check, Share2 } from 'lucide-react-native';
import { useState } from 'react';
import { ActivityIndicator, Pressable, Text } from 'react-native';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { shareWrappedCard, type Wrapped, type WrappedCard } from '@/lib/api/wrapped';
import { INK } from '@/components/wrapped/constants';

export function ShareButton({ w, card }: { w: Wrapped; card: WrappedCard }) {
  const t = useTheme();
  const tr = useT();
  const [state, setState] = useState<'idle' | 'busy' | 'copied'>('idle');
  const share = async () => {
    setState('busy');
    const outcome = await shareWrappedCard(w, card);
    setState(outcome === 'copied' ? 'copied' : 'idle');
    if (outcome === 'copied') setTimeout(() => setState('idle'), 1800);
  };
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={state === 'copied' ? tr('wrapped.linkCopied') : tr('wrapped.share')}
      accessibilityState={{ busy: state === 'busy' }}
      disabled={state === 'busy'}
      onPress={share}
      style={({ pressed }) => ({
        flexDirection: 'row',
        alignItems: 'center',
        gap: 8,
        height: 48,
        paddingHorizontal: 22,
        borderRadius: 999,
        backgroundColor: INK,
        opacity: pressed ? 0.8 : 1,
      })}>
      {state === 'busy' ? (
        <ActivityIndicator size="small" color={t.onStrong} />
      ) : state === 'copied' ? (
        <Check size={18} color={t.onStrong} />
      ) : (
        <Share2 size={18} color={t.onStrong} />
      )}
      <Text style={{ fontFamily: t.font.bold, fontSize: 16, color: t.onStrong }}>{tr(state === 'copied' ? 'wrapped.linkCopied' : 'wrapped.share')}</Text>
    </Pressable>
  );
}
