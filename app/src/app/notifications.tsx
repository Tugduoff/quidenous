import { useQueryClient } from '@tanstack/react-query';
import {
  ArrowRight,
  AtSign,
  Bell,
  BellOff,
  BellRing,
  Crown,
  type LucideIcon,
  MessageCircle,
  MessageCircleQuestion,
  Moon,
  Sparkles,
  UserPlus,
} from 'lucide-react-native';
import { type ReactNode, useRef, useState } from 'react';
import { ActivityIndicator, Linking, Platform, Pressable, Text, View } from 'react-native';
import Animated from 'react-native-reanimated';

import { BackButton, ErrorText, Section } from '@/components/auth-ui';
import { Screen } from '@/components/screen';
import { Card, Dialog, DialogTitle, Display, PressableScale, Toggle } from '@/components/ui';
import { useFeatures } from '@/lib/auth';
import { motion } from '@/lib/motion';
import { notifKey, type NotifKind, type NotifPrefs, useDevicePush, useNotifPrefs, useSaveNotifPrefs } from '@/lib/notifications';
import { useTheme } from '@/lib/theme';

const KINDS: { kind: NotifKind; label: string; Icon: LucideIcon }[] = [
  { kind: 'question', label: 'Nouvelle question', Icon: Sparkles },
  { kind: 'result', label: 'Résultat', Icon: Crown },
  { kind: 'chat', label: 'Messages', Icon: MessageCircle },
  { kind: 'mention', label: 'Mentions', Icon: AtSign },
  { kind: 'nudge', label: 'Relances', Icon: BellRing },
  { kind: 'added', label: 'Ajout à un salon', Icon: UserPlus },
  { kind: 'ask', label: 'Demandes de questions', Icon: MessageCircleQuestion },
];

const QUIET_START = 22 * 60;
const QUIET_END = 8 * 60;
const HOURS = Array.from({ length: 24 }, (_, h) => h * 60);

const hhmm = (m: number) => `${String(Math.floor(m / 60)).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`;

/** Icon, title, an optional short line (red when something needs doing), and a switch. */
function Line({
  Icon,
  on,
  title,
  sub,
  warn,
  right,
}: {
  Icon: LucideIcon;
  on: boolean;
  title: string;
  sub?: string;
  warn?: boolean;
  right: ReactNode;
}) {
  const t = useTheme();
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 12, minHeight: 36 }}>
      <Icon size={20} color={on ? t.accInk : t.mut} />
      <View style={{ flex: 1, minWidth: 0, gap: 2 }}>
        <Text numberOfLines={1} style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>
          {title}
        </Text>
        {!!sub && <Text numberOfLines={1} style={{ fontFamily: warn ? t.font.semibold : t.font.regular, fontSize: 13, color: warn ? t.tag : t.mut }}>{sub}</Text>}
      </View>
      {right}
    </View>
  );
}

/** Greyed out and untouchable while notifications are off. */
function Dim({ off, children }: { off: boolean; children: ReactNode }) {
  return (
    <View aria-disabled={off} style={{ opacity: off ? 0.4 : 1, pointerEvents: off ? 'none' : 'auto' }}>
      {children}
    </View>
  );
}

/** Profile > Notifications: on/off (this device and the account), what to be told about, quiet hours. Pushes only: the inbox keeps everything. */
export default function NotificationsScreen() {
  const t = useTheme();
  const qc = useQueryClient();
  const features = useFeatures();
  const { data: prefs, isError } = useNotifPrefs(features.notificationSettings);
  const save = useSaveNotifPrefs();
  const device = useDevicePush();
  const [enabling, setEnabling] = useState(false);
  const [enableFailed, setEnableFailed] = useState(false);
  const [picking, setPicking] = useState<'start' | 'end' | null>(null);
  // The closing dialog keeps showing what it was picking while it fades out.
  const lastPick = useRef<'start' | 'end'>('start');
  if (picking) lastPick.current = picking;
  const which = picking ?? lastPick.current;

  // From the cache, not this render: two quick taps both land.
  const set = (patch: Partial<NotifPrefs>) => {
    const current = qc.getQueryData<NotifPrefs>(notifKey);
    if (current) save.mutate({ ...current, ...patch });
  };

  const body = () => {
    if (!prefs) {
      if (isError || (features.loaded && !features.notificationSettings)) return <ErrorText>Réglages indisponibles</ErrorText>;
      return <ActivityIndicator color={t.mut} style={{ marginTop: 24 }} />;
    }
    const state = device.state;
    const needsDevice = prefs.enabled && state === 'off';
    const blocked = state === 'denied';
    const master = prefs.enabled && state !== 'off' && !blocked;
    const sub = blocked
      ? Platform.OS === 'web'
        ? 'Bloquées par le navigateur'
        : 'Bloquées dans les réglages'
      : needsDevice
        ? 'À activer sur cet appareil'
        : state === 'unsupported'
          ? 'Indisponibles ici'
          : undefined;

    const setMaster = (on: boolean) => {
      setEnableFailed(false);
      if (!on) return set({ enabled: false });
      if (!prefs.enabled) set({ enabled: true });
      // The phone won't ask again: only its settings can.
      if (blocked && Platform.OS !== 'web') return Linking.openSettings();
      if (state !== 'off' && !blocked) return;
      // Straight from the tap: a browser only asks for permission on a user gesture.
      setEnabling(true);
      device
        .enable()
        .catch(() => setEnableFailed(true))
        .finally(() => setEnabling(false));
    };

    const quiet = prefs.quiet_start != null && prefs.quiet_end != null;
    const pickedValue = which === 'start' ? prefs.quiet_start : prefs.quiet_end;
    const otherValue = which === 'start' ? prefs.quiet_end : prefs.quiet_start;

    return (
      <>
        <Card style={{ padding: 16 }}>
          <Line
            Icon={master ? Bell : BellOff}
            on={master}
            title="Notifications"
            sub={sub}
            warn={needsDevice || blocked}
            right={enabling ? <ActivityIndicator color={t.mut} /> : <Toggle label="" a11y="Notifications" value={master} onChange={setMaster} />}
          />
        </Card>
        <ErrorText>{enableFailed ? 'Activation impossible sur cet appareil' : save.isError ? 'Réglage non enregistré' : null}</ErrorText>

        <Dim off={!prefs.enabled}>
          <Section title="Me prévenir pour">
            {KINDS.map(({ kind, label, Icon }) => {
              const on = !prefs.off.includes(kind);
              return (
                <Line
                  key={kind}
                  Icon={Icon}
                  on={on}
                  title={label}
                  right={
                    <Toggle
                      label=""
                      a11y={label}
                      value={on}
                      onChange={(v) => set({ off: v ? prefs.off.filter((k) => k !== kind) : [...prefs.off, kind] })}
                    />
                  }
                />
              );
            })}
          </Section>
        </Dim>

        <Dim off={!prefs.enabled}>
          <Card style={{ padding: 16, gap: 14 }}>
            <Line
              Icon={Moon}
              on={quiet}
              title="Heures calmes"
              right={
                <Toggle
                  label=""
                  a11y="Heures calmes"
                  value={quiet}
                  onChange={(v) => set(v ? { quiet_start: QUIET_START, quiet_end: QUIET_END } : { quiet_start: null, quiet_end: null })}
                />
              }
            />
            {quiet && (
              <Animated.View entering={motion.enter} exiting={motion.exit} style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
                <TimeChip label="Début" value={prefs.quiet_start!} onPress={() => setPicking('start')} />
                <ArrowRight size={16} color={t.mut} />
                <TimeChip label="Fin" value={prefs.quiet_end!} onPress={() => setPicking('end')} />
              </Animated.View>
            )}
          </Card>
        </Dim>

        <Dialog visible={picking != null} onClose={() => setPicking(null)}>
          <DialogTitle title={which === 'end' ? 'Fin' : 'Début'} />
          <View style={{ flexDirection: 'row', flexWrap: 'wrap', gap: 8 }}>
            {HOURS.map((m) => {
              const on = m === pickedValue;
              // Start and end can't be the same hour.
              const taken = m === otherValue;
              return (
                <Pressable
                  key={m}
                  accessibilityRole="radio"
                  accessibilityState={{ selected: on, disabled: taken }}
                  disabled={taken}
                  onPress={() => {
                    if (!on) set(which === 'end' ? { quiet_end: m } : { quiet_start: m });
                    setPicking(null);
                  }}
                  style={{
                    width: '22%',
                    flexGrow: 1,
                    height: 40,
                    borderRadius: 12,
                    alignItems: 'center',
                    justifyContent: 'center',
                    backgroundColor: on ? t.acc : t.card2,
                    opacity: taken ? 0.3 : 1,
                  }}>
                  <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: on ? t.accTxt : t.txt }}>{hhmm(m)}</Text>
                </Pressable>
              );
            })}
          </View>
        </Dialog>
      </>
    );
  };

  return (
    <Screen tabs={false}>
      <BackButton />
      <Display size={30}>Notifications</Display>
      {body()}
    </Screen>
  );
}

function TimeChip({ label, value, onPress }: { label: string; value: number; onPress: () => void }) {
  const t = useTheme();
  return (
    <PressableScale
      scale={0.97}
      accessibilityRole="button"
      accessibilityLabel={`${label} : ${hhmm(value)}. Changer`}
      onPress={onPress}
      style={{ flex: 1, height: 44, borderRadius: 14, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card2 }}>
      <Text style={{ fontFamily: t.font.semibold, fontSize: 16, color: t.txt }}>{hhmm(value)}</Text>
    </PressableScale>
  );
}
