import { Share2 } from 'lucide-react-native';
import { useState } from 'react';
import { Pressable, Text, View } from 'react-native';
import QRCode from 'react-native-qrcode-svg';
import { useT } from '@/lib/i18n';
import { shareText } from '@/lib/device/share';
import { useTheme } from '@/lib/theme';
import { Card } from '@/components/ui/card';
import { CodeText } from '@/components/ui/text';
import { friendLink, friendMessage } from '@/lib/links';

export function FriendCodeCard({ code }: { code: string }) {
  const t = useTheme();
  const tr = useT();
  const [copied, setCopied] = useState(false);
  const share = async () => {
    if ((await shareText(friendMessage(code))) === 'copied') {
      setCopied(true);
      setTimeout(() => setCopied(false), 1800);
    }
  };
  return (
  <Card style={{ padding: 20, gap: 14, alignItems: 'center' }}>
    <View style={{ padding: 12, borderRadius: 16, backgroundColor: '#fff' }}>
      <QRCode value={friendLink(code)} size={168} color="#121216" backgroundColor="#fff" />
    </View>
    <View style={{ alignSelf: 'stretch', alignItems: 'center', gap: 2 }}>
      <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>{tr('friends.myCode')}</Text>
      <CodeText code={code} size={32} center />
    </View>
    <Pressable
      accessibilityRole="button"
      onPress={share}
      style={{
        alignSelf: 'stretch',
        height: 44,
        borderRadius: t.radius.pill,
        flexDirection: 'row',
        gap: 8,
        alignItems: 'center',
        justifyContent: 'center',
        backgroundColor: t.acc,
      }}>
      <Share2 size={18} color={t.accTxt} />
      <Text style={{ fontFamily: t.font.bold, fontSize: 15, color: t.accTxt }}>{copied ? tr('friends.linkCopied') : tr('friends.shareCode')}</Text>
    </Pressable>
  </Card>
  );
}
