import { Check, Download, RefreshCw, Settings, Sparkles, X } from 'lucide-react-native';
import { usePathname } from 'expo-router';
import { useEffect } from 'react';
import { ActivityIndicator, Pressable, ScrollView, Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import { Dialog, PressableScale } from '@/components/ui';
import { motion } from '@/lib/motion';
import { restart, useOtaReady } from '@/lib/ota';
import { appVersion, checkForUpdate, clearCheck, install, later, openInstallSettings, UPDATES, useUpdate } from '@/lib/update';
import { useTheme } from '@/lib/theme';

const megabytes = (bytes: number) => `${Math.round(bytes / 1_000_000)} Mo`;

/** A newer APK is out: its notes, Installer (download, then Android's installer) or Plus tard. Mounted once, in the root layout. */
export function UpdateDialog() {
  const t = useTheme();
  const { release, open, status, progress } = useUpdate();
  if (!UPDATES || !release) return null;
  const downloading = status === 'download';
  return (
    <Dialog visible={open} onClose={later}>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 12 }}>
        <View style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.accSoft }}>
          <Download size={20} color={t.accInk} />
        </View>
        <View style={{ flex: 1 }}>
          <Text style={{ fontFamily: t.font.bold, fontSize: 18, color: t.txt }}>Mise à jour</Text>
          <Text style={{ fontFamily: t.font.regular, fontSize: 14, color: t.mut }}>
            {release.version} · {megabytes(release.size)}
          </Text>
        </View>
      </View>

      <Notes text={release.notes} />

      {downloading && (
        <View style={{ height: 6, borderRadius: 3, overflow: 'hidden', backgroundColor: t.card2 }}>
          <View style={{ width: `${Math.round(progress * 100)}%`, height: 6, borderRadius: 3, backgroundColor: t.acc }} />
        </View>
      )}

      {status === 'blocked' && (
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
          <Text style={{ flex: 1, fontFamily: t.font.regular, fontSize: 14, lineHeight: 19, color: t.mut }}>
            Bloquée ? Autorise l’app à installer des mises à jour.
          </Text>
          <PressableScale
            accessibilityRole="button"
            onPress={openInstallSettings}
            style={{
              flexDirection: 'row',
              alignItems: 'center',
              gap: 6,
              height: 36,
              paddingHorizontal: 12,
              borderRadius: t.radius.pill,
              backgroundColor: t.card2,
            }}>
            <Settings size={16} color={t.txt} />
            <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.txt }}>Réglages</Text>
          </PressableScale>
        </View>
      )}

      {status === 'error' && <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.tag }}>Téléchargement impossible</Text>}

      <View style={{ flexDirection: 'row', gap: 10, marginTop: 6 }}>
        <PressableScale
          accessibilityRole="button"
          onPress={later}
          style={{ flex: 1, height: 46, borderRadius: t.radius.pill, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card2 }}>
          <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>Plus tard</Text>
        </PressableScale>
        <PressableScale
          accessibilityRole="button"
          accessibilityState={{ disabled: downloading }}
          disabled={downloading}
          onPress={install}
          style={{
            flex: 1,
            height: 46,
            borderRadius: t.radius.pill,
            flexDirection: 'row',
            gap: 8,
            alignItems: 'center',
            justifyContent: 'center',
            backgroundColor: t.acc,
          }}>
          {downloading ? (
            <Text style={{ fontFamily: t.font.bold, fontSize: 15, color: t.accTxt, fontVariant: ['tabular-nums'] }}>
              {Math.round(progress * 100)} %
            </Text>
          ) : (
            <>
              <Download size={18} color={t.accTxt} />
              <Text style={{ fontFamily: t.font.bold, fontSize: 15, color: t.accTxt }}>Installer</Text>
            </>
          )}
        </PressableScale>
      </View>
    </Dialog>
  );
}

/** The release notes (GitHub markdown, as written by the release job): one row per line, "- " lines as bullets. */
function Notes({ text }: { text: string }) {
  const t = useTheme();
  const lines = text
    .split('\n')
    .map((l) => l.trim())
    .filter((l) => l && !/^#+\s*$/.test(l));
  if (!lines.length) return null;
  return (
    <ScrollView style={{ maxHeight: 220 }} contentContainerStyle={{ gap: 8 }}>
      {lines.map((line, i) => {
        const bullet = /^[-*•]\s+/.test(line);
        const words = line.replace(/^[-*•]\s+|^#+\s+/, '').replace(/\*\*|__|`/g, '');
        return (
          <View key={i} style={{ flexDirection: 'row', gap: 10 }}>
            {bullet && <View style={{ width: 5, height: 5, borderRadius: 3, marginTop: 8, backgroundColor: t.accInk }} />}
            <Text style={{ flex: 1, fontFamily: t.font.regular, fontSize: 15, lineHeight: 21, color: t.txt }}>{words}</Text>
          </View>
        );
      })}
    </ScrollView>
  );
}

/** Bottom of the profile: the installed version and, on the APK, a manual check. */
export function VersionLine() {
  const t = useTheme();
  const { status, release } = useUpdate();
  // A manual check's answer stays a few seconds.
  useEffect(() => {
    if (status !== 'latest' && status !== 'failed') return;
    const timer = setTimeout(clearCheck, 4000);
    return () => clearTimeout(timer);
  }, [status]);
  const small = { fontFamily: t.font.semibold, fontSize: 13, color: t.mut };
  const Icon = status === 'latest' ? Check : release ? Download : RefreshCw;
  const label =
    status === 'checking'
      ? 'Vérification…'
      : status === 'latest'
        ? 'À jour'
        : status === 'failed'
          ? 'Vérification impossible'
          : release
            ? `Mise à jour ${release.version}`
            : 'Vérifier les mises à jour';
  return (
    <View style={{ alignItems: 'center', gap: 2 }}>
      <Text style={small}>Version {appVersion()}</Text>
      {UPDATES && (
        <Pressable
          accessibilityRole="button"
          disabled={status === 'checking'}
          onPress={() => checkForUpdate(true)}
          hitSlop={6}
          style={{ flexDirection: 'row', alignItems: 'center', gap: 6, padding: 6 }}>
          {status === 'checking' ? <ActivityIndicator size="small" color={t.mut} /> : <Icon size={14} color={release ? t.accInk : t.mut} />}
          <Text style={[small, release && status === 'idle' && { color: t.accInk }]}>{label}</Text>
        </Pressable>
      )}
    </View>
  );
}

const TABS = ['/', '/history', '/stats', '/questions'];

/** An over-the-air update is downloaded: it applies on the next start, or now with Relancer. Floats above the tab bar (tabs only). */
export function OtaToast() {
  const t = useTheme();
  const insets = useSafeAreaInsets();
  const pathname = usePathname();
  const { ready, dismiss } = useOtaReady();
  if (!ready || !TABS.includes(pathname)) return null;
  return (
    <Animated.View
      entering={motion.rise}
      exiting={motion.exit}
      style={{
        position: 'absolute',
        left: 16,
        right: 16,
        bottom: insets.bottom + 72,
        flexDirection: 'row',
        alignItems: 'center',
        gap: 10,
        paddingLeft: 14,
        paddingRight: 6,
        paddingVertical: 6,
        borderRadius: t.radius.pill,
        backgroundColor: t.card,
        borderWidth: 1,
        borderColor: t.line,
        ...t.shadow,
      }}>
      <Sparkles size={18} color={t.accInk} />
      <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 14, color: t.txt }}>
        Nouvelle version prête
      </Text>
      <PressableScale
        accessibilityRole="button"
        onPress={restart}
        style={{ height: 34, paddingHorizontal: 14, borderRadius: t.radius.pill, alignItems: 'center', justifyContent: 'center', backgroundColor: t.acc }}>
        <Text style={{ fontFamily: t.font.bold, fontSize: 14, color: t.accTxt }}>Relancer</Text>
      </PressableScale>
      <Pressable
        accessibilityRole="button"
        accessibilityLabel="Masquer"
        onPress={dismiss}
        hitSlop={6}
        style={{ width: 34, height: 34, alignItems: 'center', justifyContent: 'center' }}>
        <X size={18} color={t.mut} />
      </Pressable>
    </Animated.View>
  );
}
