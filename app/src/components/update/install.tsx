import { ChevronRight, Download, Share, SquarePlus, Smartphone, X } from 'lucide-react-native';
import { type ComponentType, useState } from 'react';
import { Platform, Pressable, Text, View } from 'react-native';
import QRCode from 'react-native-qrcode-svg';
import { Screen } from '@/components/ui/screen';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { CodeText, Display, Logo } from '@/components/ui/text';
import { presetFriendCode, presetJoinCode } from '@/lib/device/launch-params';
import { useInstallPrompt } from '@/lib/hooks/use-install-prompt';

const web = Platform.OS === 'web' && typeof window !== 'undefined';

export function installMode(): 'none' | 'banner' | 'block' {
  if (!web || __DEV__) return 'none';
  if (location.pathname.startsWith('/party/')) return 'none';
  const standalone =
    window.matchMedia('(display-mode: standalone)').matches ||
    window.matchMedia('(display-mode: fullscreen)').matches ||
    (navigator as Navigator & { standalone?: boolean }).standalone === true;
  if (standalone) return 'none';
  return device() === 'desktop' ? 'block' : 'banner';
}

export function InstallBanner({ onOpen, onClose }: { onOpen: () => void; onClose: () => void }) {
  const t = useTheme();
  const tr = useT();
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 4, paddingLeft: 16, paddingRight: 6, backgroundColor: t.acc }}>
      <Pressable
        accessibilityRole="button"
        onPress={onOpen}
        style={{ flex: 1, flexDirection: 'row', alignItems: 'center', gap: 8, paddingVertical: 10 }}>
        <Smartphone size={18} color={t.accTxt} />
        <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 14, color: t.accTxt }}>
          {tr('install.banner')}
        </Text>
        <ChevronRight size={18} color={t.accTxt} />
      </Pressable>
      <Pressable
        accessibilityRole="button"
        accessibilityLabel={tr('install.hide')}
        onPress={onClose}
        hitSlop={8}
        style={{ width: 36, height: 36, alignItems: 'center', justifyContent: 'center' }}>
        <X size={18} color={t.accTxt} />
      </Pressable>
    </View>
  );
}

function device(): 'ios' | 'android' | 'desktop' {
  const ua = navigator.userAgent;
  if (/iPhone|iPad|iPod/.test(ua) || (/Macintosh/.test(ua) && navigator.maxTouchPoints > 1)) return 'ios';
  if (/Android/.test(ua)) return 'android';
  return 'desktop';
}

export function InstallScreen({ onClose }: { onClose?: () => void }) {
  const t = useTheme();
  const tr = useT();
  const [kind] = useState(device);
  const { canPrompt, installed, install } = useInstallPrompt();
  const code = presetJoinCode();
  const friend = presetFriendCode();
  const shownCode = code || friend;

  const text = { fontFamily: t.font.regular, fontSize: 16, lineHeight: 22, color: t.mut };
  return (
    <Screen tabs={false}>
      {onClose && (
        <Pressable
          accessibilityRole="button"
          accessibilityLabel={tr('common.close')}
          onPress={onClose}
          hitSlop={4}
          style={{ alignSelf: 'flex-end', width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
          <X size={20} color={t.txt} />
        </Pressable>
      )}
      <View style={{ marginTop: onClose ? 0 : 32, gap: 14 }}>
        <Logo size={40} />
        <Display size={30}>{kind === 'desktop' ? tr('install.desktopTitle') : tr('install.title')}</Display>
        <Text style={text}>
          {kind === 'desktop' ? tr('install.desktopSub') : tr('install.sub')}
        </Text>
      </View>

      {kind === 'desktop' && (
        <Card style={{ padding: 20, alignItems: 'center' }}>
          <View style={{ padding: 12, borderRadius: 16, backgroundColor: t.qr.paper }}>
            <QRCode value={window.location.href} size={200} color={t.qr.ink} backgroundColor={t.qr.paper} />
          </View>
        </Card>
      )}

      {kind === 'ios' && (
        <Card style={{ padding: 18, gap: 18 }}>
          <Step n={1} Icon={Share} label={tr('install.ios.share')} />
          <Step n={2} Icon={SquarePlus} label={tr('install.ios.addToHome')} />
          <Step n={3} Icon={Smartphone} label={tr('install.open')} />
        </Card>
      )}

      {kind === 'android' &&
        (installed ? (
          <Card style={{ padding: 18 }}>
            <Step n={1} Icon={Smartphone} label={tr('install.android.installed')} />
          </Card>
        ) : canPrompt ? (
          <Button onPress={install}>
            <Download size={20} color={t.accTxt} />
            <Text style={{ fontFamily: t.font.bold, fontSize: 17, color: t.accTxt }}>{tr('install.android.install')}</Text>
          </Button>
        ) : (
          <Card style={{ padding: 18, gap: 18 }}>
            <Step n={1} Icon={Download} label={tr('install.android.menu')} />
            <Step n={2} Icon={SquarePlus} label={tr('install.android.tapInstall')} />
            <Step n={3} Icon={Smartphone} label={tr('install.open')} />
          </Card>
        ))}

      {kind !== 'desktop' && shownCode && (
        <Card style={{ padding: 18, gap: 6 }}>
          <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>
            {code ? tr('install.inviteCode') : tr('install.friendCode')}
          </Text>
          <CodeText code={shownCode} size={32} />
        </Card>
      )}
    </Screen>
  );
}

function Step({ n, Icon, label }: { n: number; Icon: ComponentType<{ size?: number; color?: string }>; label: string }) {
  const t = useTheme();
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 14 }}>
      <View style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card2 }}>
        <Icon size={20} color={t.txt} />
      </View>
      <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, lineHeight: 20, color: t.txt }}>
        {n}. {label}
      </Text>
    </View>
  );
}
