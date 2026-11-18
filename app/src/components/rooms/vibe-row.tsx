import { Check, ChevronRight } from 'lucide-react-native';
import { useState } from 'react';
import { Pressable, Text } from 'react-native';
import { VibeCards, VibeIcon, vibeName } from '@/components/rooms/vibes';
import { SettingsRow } from '@/components/rooms/settings-section';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { Dialog, DialogTitle } from '@/components/ui/dialog';
import { useVibes } from '@/lib/api/group';
import { useFeatures } from '@/lib/api/features';

/** The tone of the room's built-in questions (a server that has vibes). */
export function VibeRow({ value, onChange }: { value: string | null; onChange: (vibe: string | null) => void }) {
  const t = useTheme();
  const tr = useT();
  const features = useFeatures();
  const { data: vibes } = useVibes(features.vibes);
  const [open, setOpen] = useState(false);
  if (!features.vibes || !vibes?.length) return null;
  const current = vibes.find((v) => v.id === value);
  const pick = (v: string | null) => {
    setOpen(false);
    if (v !== value) onChange(v);
  };
  return (
    <>
      <SettingsRow
        label={tr('room.settings.vibe')}
        a11y={tr('room.settings.vibeA11y', { name: current ? vibeName(current) : tr('room.settings.vibeNoneLower') })}
        onPress={() => setOpen(true)}
        right={
          <>
            {current && <VibeIcon name={current?.icon} size={16} color={t.accInk} />}
            <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.semibold, fontSize: 14, color: current ? t.txt : t.mut }}>
              {current ? vibeName(current) : tr('room.settings.vibeNone')}
            </Text>
            <ChevronRight size={18} color={t.mut} />
          </>
        }
      />
      <Dialog visible={open} onClose={() => setOpen(false)}>
        <DialogTitle title={tr('room.settings.vibe')} />
        <VibeCards vibes={vibes} value={value} onChange={pick} />
        <Pressable
          accessibilityRole="radio"
          accessibilityState={{ selected: !current }}
          onPress={() => pick(null)}
          style={{ height: 44, flexDirection: 'row', gap: 6, alignItems: 'center', justifyContent: 'center' }}>
          {!current && <Check size={16} color={t.accInk} />}
          <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: current ? t.mut : t.accInk }}>{tr('room.settings.vibeNone')}</Text>
        </Pressable>
      </Dialog>
    </>
  );
}
