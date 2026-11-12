import { Download, Share, SquarePlus, Smartphone } from 'lucide-react-native';
import { type ComponentType, useEffect, useState } from 'react';
import { Platform, Text, View } from 'react-native';
import QRCode from 'react-native-qrcode-svg';

import { Screen } from '@/components/screen';
import { Button, Card, Display, Logo } from '@/components/ui';
import { presetFriendCode, presetJoinCode } from '@/lib/api';
import { useTheme } from '@/lib/theme';

type InstallPrompt = Event & { prompt: () => Promise<void>; userChoice: Promise<{ outcome: 'accepted' | 'dismissed' }> };

const web = Platform.OS === 'web' && typeof window !== 'undefined';

// Android fires it once, early: keep it for the Installer button.
let deferred: InstallPrompt | null = null;
const waiting = new Set<() => void>();
if (web) {
  window.addEventListener('beforeinstallprompt', (e) => {
    e.preventDefault();
    deferred = e as InstallPrompt;
    waiting.forEach((f) => f());
  });
}

/**
 * The web build only runs as the home screen app (push, full screen, its own session):
 * in a browser tab it shows how to install it. Not in development, to keep `expo start --web`.
 */
export function mustInstall() {
  if (!web || __DEV__) return false;
  const standalone =
    window.matchMedia('(display-mode: standalone)').matches ||
    window.matchMedia('(display-mode: fullscreen)').matches ||
    (navigator as Navigator & { standalone?: boolean }).standalone === true;
  return !standalone;
}

function device(): 'ios' | 'android' | 'desktop' {
  const ua = navigator.userAgent;
  // iPadOS says it is a Mac: tell them apart by the touch screen.
  if (/iPhone|iPad|iPod/.test(ua) || (/Macintosh/.test(ua) && navigator.maxTouchPoints > 1)) return 'ios';
  if (/Android/.test(ua)) return 'android';
  return 'desktop';
}

export function InstallScreen() {
  const t = useTheme();
  const [kind] = useState(device);
  const [prompt, setPrompt] = useState(deferred);
  const [installed, setInstalled] = useState(false);
  const code = presetJoinCode();
  const friend = presetFriendCode();

  useEffect(() => {
    const ready = () => setPrompt(deferred);
    const done = () => setInstalled(true);
    waiting.add(ready);
    window.addEventListener('appinstalled', done);
    return () => {
      waiting.delete(ready);
      window.removeEventListener('appinstalled', done);
    };
  }, []);

  const install = async () => {
    if (!prompt) return;
    await prompt.prompt();
    if ((await prompt.userChoice).outcome === 'accepted') setInstalled(true);
    deferred = null;
    setPrompt(null);
  };

  const text = { fontFamily: t.font.regular, fontSize: 16, lineHeight: 22, color: t.mut };
  return (
    <Screen tabs={false}>
      <View style={{ marginTop: 32, gap: 14 }}>
        <Logo size={40} />
        <Display size={30}>{kind === 'desktop' ? 'Ça se joue sur téléphone' : "Installe l'app"}</Display>
        <Text style={text}>
          {kind === 'desktop'
            ? 'Scanne ce QR code avec ton téléphone pour ouvrir qui de nous ? dessus.'
            : "qui de nous ? marche depuis ton écran d'accueil : notifications, plein écran, et tu restes connecté."}
        </Text>
      </View>

      {kind === 'desktop' && (
        <Card style={{ padding: 20, alignItems: 'center' }}>
          <View style={{ padding: 12, borderRadius: 16, backgroundColor: '#fff' }}>
            <QRCode value={window.location.href} size={200} color="#121216" backgroundColor="#fff" />
          </View>
        </Card>
      )}

      {kind === 'ios' && (
        <Card style={{ padding: 18, gap: 18 }}>
          <Step n={1} Icon={Share} label="Touche Partager dans la barre du navigateur" />
          <Step n={2} Icon={SquarePlus} label="Choisis « Sur l'écran d'accueil », puis Ajouter" />
          <Step n={3} Icon={Smartphone} label="Ouvre qui de nous ? depuis ton écran d'accueil" />
        </Card>
      )}

      {kind === 'android' &&
        (installed ? (
          <Card style={{ padding: 18 }}>
            <Step n={1} Icon={Smartphone} label="C'est installé : ouvre qui de nous ? depuis ton écran d'accueil" />
          </Card>
        ) : prompt ? (
          <Button onPress={install}>
            <Download size={20} color={t.accTxt} />
            <Text style={{ fontFamily: t.font.bold, fontSize: 17, color: t.accTxt }}>Installer</Text>
          </Button>
        ) : (
          <Card style={{ padding: 18, gap: 18 }}>
            <Step n={1} Icon={Download} label="Ouvre le menu ⋮ du navigateur" />
            <Step n={2} Icon={SquarePlus} label="Choisis « Installer l'application » ou « Ajouter à l'écran d'accueil »" />
            <Step n={3} Icon={Smartphone} label="Ouvre qui de nous ? depuis ton écran d'accueil" />
          </Card>
        ))}

      {kind !== 'desktop' && (code || friend) && (
        <Card style={{ padding: 18, gap: 6 }}>
          <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>
            {code ? "Ton code d'invitation, à entrer une fois dans l'app" : "Le code ami, à entrer une fois dans l'app"}
          </Text>
          <Text selectable style={{ fontFamily: t.font.display, fontSize: 32, letterSpacing: 4, color: t.txt }}>
            {code ?? friend}
          </Text>
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
