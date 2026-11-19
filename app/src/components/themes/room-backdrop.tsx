import { memo, useState } from 'react';
import { useWindowDimensions, View } from 'react-native';

import { SceneArt } from '@/components/themes/scene-art';
import { useRoomScene, useScheme } from '@/lib/theme';
import { SCENE_H } from '@/lib/utils/room-art';

const SCREEN_PAD = 20;

export const RoomBackdrop = memo(function RoomBackdrop() {
  const id = useRoomScene();
  const dark = useScheme() === 'dark';
  const win = useWindowDimensions().width;
  const [measured, setMeasured] = useState(0);
  if (!id) return null;
  return (
    <View
      pointerEvents="none"
      aria-hidden
      onLayout={(e) => setMeasured(Math.round(e.nativeEvent.layout.width))}
      style={{ position: 'absolute', top: -SCREEN_PAD, left: -SCREEN_PAD, right: -SCREEN_PAD, height: SCENE_H, overflow: 'hidden' }}>
      <SceneArt id={id} dark={dark} width={measured || win} />
    </View>
  );
});
