import { View } from 'react-native';
import { type Avatar, type Field, optionCount, PRESETS, presetAvatar } from '@/lib/utils/avatar';
import { useT } from '@/lib/i18n';
import { AvatarChoice } from '@/components/avatar/avatar-choice';
import { TABS } from '@/components/avatar/avatar-tabs';

export function AvatarOptions({
  avatar,
  spec,
  tab,
  cell,
  onAvatar,
  onPart,
}: {
  avatar: Avatar;
  spec: string;
  tab: number;
  cell: number;
  onAvatar: (a: Avatar) => void;
  onPart: (f: Field, i: number) => void;
}) {
  const tr = useT();
  const { field, colors, zoom } = TABS[tab];
  return (
  <View style={{ flexDirection: 'row', flexWrap: 'wrap', gap: 10 }}>
    {!field && !colors
      ? PRESETS.map((p, i) => {
          const a = presetAvatar(p);
          return (
            <AvatarChoice key={p} avatar={a} on={p === spec} onPress={() => onAvatar(a)} width={cell} label={tr('avatar.preset', { n: i + 1 })} />
          );
        })
      : field &&
        Array.from({ length: optionCount(field) }, (_, i) => (
          <AvatarChoice
            key={i}
            avatar={{ ...avatar, [field]: i }}
            on={avatar[field] === i}
            onPress={() => onPart(field, i)}
            width={cell}
            zoom={zoom}
            label={`${tr(TABS[tab].label)} ${i + 1}`}
          />
        ))}
  </View>
  );
}
