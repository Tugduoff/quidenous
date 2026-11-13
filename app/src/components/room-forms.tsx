import { useMutation } from '@tanstack/react-query';
import { ChevronLeft, LogIn, Plus } from 'lucide-react-native';
import { useState } from 'react';
import { Pressable, Text, TextInput, View } from 'react-native';
import Animated from 'react-native-reanimated';

import { ScanButton } from '@/components/qr-scanner';
import { Button, Card, Display, Segmented } from '@/components/ui';
import { VibeCards } from '@/components/vibes';
import { api, ApiError } from '@/lib/api';
import { authError, useFeatures } from '@/lib/auth';
import { getLang, type Lang, type TKey, useT } from '@/lib/i18n';
import { motion } from '@/lib/motion';
import { useVibes } from '@/lib/queries';
import { useTheme } from '@/lib/theme';

const ERRORS: Record<string, TKey> = {
  bad_code: 'room.errors.badCode',
  removed: 'room.errors.removed',
  bad_room_name: 'room.errors.badName',
  bad_vibe: 'room.errors.badVibe',
  bad_language: 'room.errors.badLanguage',
};

/**
 * Join a room with a code (or its QR code), or create one. `joinFirst` puts the code field on top.
 * Creating takes a second step to pick the room's vibe (a server that has them); `onVibeStep` tells the screen, so it can clear the way.
 */
export function RoomForms({
  initialCode,
  onEntered,
  joinFirst,
  onVibeStep,
  createOnly,
}: {
  initialCode?: string;
  onEntered: (id: number) => void;
  joinFirst?: boolean;
  onVibeStep?: (on: boolean) => void;
  /** Only the create form, under this title (no code field): a room made for something, like a shared question. */
  createOnly?: string;
}) {
  const t = useTheme();
  const tr = useT();
  const [name, setName] = useState('');
  const [code, setCode] = useState(initialCode ?? '');
  const [error, setError] = useState<string | null>(null);
  const fail = (e: unknown) => setError(e instanceof ApiError && ERRORS[e.code] ? tr(ERRORS[e.code]) : authError(e));
  const features = useFeatures();
  const { data: vibes } = useVibes(features.vibes);
  const [picking, setPicking] = useState(false);
  const [vibe, setVibe] = useState<string | null>(null);
  // The language of the room's built-in questions: the one I use the app in, unless I pick the other.
  const [language, setLanguage] = useState<Lang>(getLang);
  const showVibes = (on: boolean) => {
    setPicking(on);
    setError(null);
    onVibeStep?.(on);
  };

  const create = useMutation({
    mutationFn: (withVibe: string | null) =>
      api<{ id: number }>('/rooms', {
        method: 'POST',
        json: { name: name.trim(), ...(withVibe ? { vibe: withVibe } : {}), ...(features.languages ? { language } : {}) },
      }),
    onSuccess: (r) => onEntered(r.id),
    onError: fail,
  });
  // Named: on to the vibes when there are some to pick from, else straight in.
  const next = () => {
    if (!name.trim() || create.isPending) return;
    if (vibes?.length) showVibes(true);
    else create.mutate(null);
  };
  const join = useMutation({
    mutationFn: (value: string) => api<{ id: number }>('/rooms/join', { method: 'POST', json: { code: value.trim() } }),
    onSuccess: (r) => onEntered(r.id),
    onError: fail,
  });
  const label = { fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4 };
  const round = (on: boolean) => ({
    width: 44,
    height: 44,
    borderRadius: 22,
    alignItems: 'center' as const,
    justifyContent: 'center' as const,
    backgroundColor: on ? t.acc : t.card2,
  });

  const createForm = (
    <>
      <Text style={[label, { marginTop: 8 }]}>{createOnly ?? tr('room.create.title')}</Text>
      <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 8, padding: 6, paddingLeft: 16 }}>
        <TextInput
          accessibilityLabel={tr('room.create.name')}
          placeholder={tr('room.create.placeholder')}
          placeholderTextColor={t.mut}
          value={name}
          onChangeText={setName}
          maxLength={40}
          returnKeyType="go"
          onSubmitEditing={next}
          style={{ flex: 1, minWidth: 0, height: 44, fontFamily: t.font.regular, fontSize: 16, color: t.txt }}
        />
        <Pressable
          accessibilityRole="button"
          accessibilityLabel={tr('room.create.a11y')}
          disabled={!name.trim() || create.isPending}
          onPress={next}
          style={round(!!name.trim())}>
          <Plus size={20} color={name.trim() ? t.accTxt : t.mut} />
        </Pressable>
      </Card>
    </>
  );
  const joinForm = (
    <>
      <Text style={[label, joinFirst ? undefined : { marginTop: 8 }]}>{tr('room.join.title')}</Text>
      <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 8, padding: 6, paddingLeft: 16 }}>
        <TextInput
          accessibilityLabel={tr('room.invite.code')}
          placeholder="ABC234"
          placeholderTextColor={t.mut}
          value={code}
          onChangeText={(v) => setCode(v.toUpperCase())}
          autoCapitalize="characters"
          autoCorrect={false}
          maxLength={20}
          returnKeyType="go"
          onSubmitEditing={() => code.trim() && !join.isPending && join.mutate(code)}
          style={{ flex: 1, minWidth: 0, height: 44, fontFamily: t.font.bold, fontSize: 16, letterSpacing: 2, color: t.txt }}
        />
        <ScanButton
          onScan={(s) => {
            if (!s.code) return setError(tr('room.join.notRoomQr'));
            setCode(s.code);
            join.mutate(s.code);
          }}
        />
        <Pressable
          accessibilityRole="button"
          accessibilityLabel={tr('common.join')}
          disabled={!code.trim() || join.isPending}
          onPress={() => join.mutate(code)}
          style={round(!!code.trim())}>
          <LogIn size={20} color={code.trim() ? t.accTxt : t.mut} />
        </Pressable>
      </Card>
    </>
  );

  const errorLine = error && <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.tag, marginLeft: 4 }}>{error}</Text>;

  if (picking && vibes)
    return (
      <Animated.View entering={motion.enter} style={{ gap: 16 }}>
        <Pressable
          accessibilityRole="button"
          accessibilityLabel={tr('room.create.backToName')}
          onPress={() => showVibes(false)}
          hitSlop={8}
          style={{ flexDirection: 'row', alignItems: 'center', gap: 4, alignSelf: 'flex-start' }}>
          <ChevronLeft size={18} color={t.mut} />
          <Text numberOfLines={1} style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.mut, maxWidth: 240 }}>
            {name.trim()}
          </Text>
        </Pressable>
        <Display size={28}>{tr('room.create.vibeTitle')}</Display>
        <VibeCards vibes={vibes} value={vibe} onChange={setVibe} />
        {features.languages && (
          <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', gap: 12 }}>
            <Text numberOfLines={1} style={[label, { flexShrink: 1 }]}>
              {tr('room.language.label')}
            </Text>
            <Segmented
              label={tr('room.language.label')}
              value={language}
              onChange={setLanguage}
              options={[
                { id: 'fr', name: tr('profile.language.fr') },
                { id: 'en', name: tr('profile.language.en') },
              ]}
            />
          </View>
        )}
        {errorLine}
        <View style={{ gap: 6 }}>
          <Button disabled={!vibe} loading={create.isPending && create.variables != null} onPress={() => vibe && create.mutate(vibe)}>
            {tr('room.create.create')}
          </Button>
          <Pressable
            accessibilityRole="button"
            disabled={create.isPending}
            onPress={() => create.mutate(null)}
            style={{ height: 44, alignItems: 'center', justifyContent: 'center' }}>
            <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.mut }}>{tr('room.create.skip')}</Text>
          </Pressable>
        </View>
      </Animated.View>
    );

  if (createOnly)
    return (
      <>
        {createForm}
        {errorLine}
      </>
    );
  return (
    <>
      {joinFirst ? joinForm : createForm}
      {joinFirst ? createForm : joinForm}
      {errorLine}
    </>
  );
}
