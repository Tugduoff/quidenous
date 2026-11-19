import { Check, ExternalLink, Monitor, Share2 } from 'lucide-react-native';
import { useState } from 'react';
import { Linking, Text } from 'react-native';
import { Card } from '@/components/ui/card';
import { PressableScale } from '@/components/ui/pressable-scale';
import { shareText } from '@/lib/device/share';
import { useT } from '@/lib/i18n';
import { partyScreenLink } from '@/lib/links';
import { useTheme } from '@/lib/theme';

export function ScreenLink({ code }: { code: string }) {
  const t = useTheme();
  const tr = useT();
  const [copied, setCopied] = useState(false);
  const url = partyScreenLink(code);
  return (
    <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, paddingVertical: 8, paddingLeft: 16, paddingRight: 8 }}>
      <Monitor size={18} color={t.txt} />
      <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>
        {tr('party.lobby.screen')}
      </Text>
      <PressableScale
        accessibilityRole="button"
        accessibilityLabel={copied ? tr('party.lobby.linkCopied') : tr('party.lobby.screenLink')}
        onPress={async () => {
          if ((await shareText(url)) !== 'copied') return;
          setCopied(true);
          setTimeout(() => setCopied(false), 1800);
        }}
        hitSlop={4}
        style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card2 }}>
        {copied ? <Check size={18} color={t.accInk} /> : <Share2 size={18} color={t.txt} />}
      </PressableScale>
      <PressableScale
        accessibilityRole="link"
        accessibilityLabel={tr('party.lobby.openScreen')}
        onPress={() => Linking.openURL(url).catch(() => {})}
        hitSlop={4}
        style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card2 }}>
        <ExternalLink size={18} color={t.txt} />
      </PressableScale>
    </Card>
  );
}
