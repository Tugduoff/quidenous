import { useQueryClient } from '@tanstack/react-query';
import { router } from 'expo-router';
import { Dices } from 'lucide-react-native';
import { useState } from 'react';
import { Text, useWindowDimensions, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { AvatarArt } from '@/components/ui/avatar-art';
import { Screen } from '@/components/ui/screen';
import { type Avatar, type Field, formatAvatar, randomAvatar, startingAvatar } from '@/lib/utils/avatar';
import { useT } from '@/lib/i18n';
import { usePop } from '@/lib/motion';
import { useSession } from '@/lib/session';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Button } from '@/components/ui/button';
import { BackButton } from '@/components/ui/back-button';
import { groupKeys } from '@/lib/api/group';
import { setMyAvatar } from '@/lib/api/me';
import { AvatarCircle } from '@/components/avatar/avatar-choice';
import { AvatarSwatches } from '@/components/avatar/avatar-swatches';
import { AvatarTabs, TABS } from '@/components/avatar/avatar-tabs';
import { AvatarOptions } from '@/components/avatar/avatar-options';

export default function AvatarBuilder() {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const { me, setMe } = useSession();
  const { width: screen } = useWindowDimensions();
  const [avatar, setAvatar] = useState<Avatar>(() => startingAvatar(me?.avatar, me?.color));
  const [tab, setTab] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(false);
  const spec = formatAvatar(avatar);
  const pop = usePop(spec, 1.06);

  const content = Math.min(screen, 560) - 40;
  const cols = Math.max(4, Math.floor(content / 100));
  const cell = Math.floor((content - (cols - 1) * 10) / cols);
  const set = (f: Field, i: number) => setAvatar((a) => ({ ...a, [f]: i }));
  const { colors } = TABS[tab];

  const save = async () => {
    setBusy(true);
    setError(false);
    try {
      setMe(await setMyAvatar(spec));
      qc.invalidateQueries({ queryKey: groupKeys.all });
      router.back();
    } catch {
      setError(true);
      setBusy(false);
    }
  };

  return (
    <Screen
      tabs={false}
      footer={
        <>
          {error && <Text style={{ fontFamily: t.font.semibold, color: t.tagInk, textAlign: 'center' }}>{tr('avatar.notSaved')}</Text>}
          <Button onPress={save} loading={busy} disabled={spec === me?.avatar && !me?.photo_url}>
            {tr('common.save')}
          </Button>
        </>
      }>
      <View style={{ flexDirection: 'row', justifyContent: 'space-between' }}>
        <BackButton onPress={() => router.back()} />
        <PressableScale
          accessibilityRole="button"
          accessibilityLabel={tr('avatar.random')}
          onPress={() => setAvatar(randomAvatar())}
          style={{
            flexDirection: 'row',
            alignItems: 'center',
            gap: 8,
            height: 40,
            paddingHorizontal: 16,
            borderRadius: 20,
            backgroundColor: t.card,
          }}>
          <Dices size={18} color={t.txt} />
          <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{tr('avatar.random')}</Text>
        </PressableScale>
      </View>

      <Animated.View style={[{ alignSelf: 'center' }, pop]}>
        <AvatarCircle size={168}>
          <AvatarArt avatar={avatar} size={168} />
        </AvatarCircle>
      </Animated.View>

      <AvatarTabs tab={tab} onTab={setTab} />

      {colors && <AvatarSwatches field={colors} value={avatar[colors]} onPick={(i) => set(colors, i)} />}

      <AvatarOptions avatar={avatar} spec={spec} tab={tab} cell={cell} onAvatar={setAvatar} onPart={set} />
    </Screen>
  );
}
