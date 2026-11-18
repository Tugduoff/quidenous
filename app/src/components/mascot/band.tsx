import { memo, useState } from 'react';
import { type StyleProp, View, type ViewStyle } from 'react-native';

import { Mascot } from '@/components/mascot/mascot';
import type { Pose, Who } from '@/lib/utils/mascot';

type Motion = 'caught' | 'accuse' | 'happy' | 'celebrate';
type Cast = [Who, Pose | 'caught', number, number, number, number, Motion?][];
const ACCUSE: Cast = [
  ['pave', { eyes: 'squint', brows: 'sus', mouth: 'smirk', arms: ['rest', 'point'], legs: ['wide', 'wide'], tilt: 4, dx: -10 }, 225, 0, 6, 6],
  ['zeste', { eyes: 'squint', brows: 'up', mouth: 'grin', arms: ['rest', 'aim'], legs: ['wide', 'wide'], tilt: 6, dx: -8 }, 200, 140, 74, 4],
  ['bulle', 'caught', 260, 280, 10, 3],
  ['paillette', { eyes: 'open', brows: 'up', mouth: 'gasp', arms: ['aim', 'rest'], legs: ['tiptoe', 'tiptoe'], tilt: -6, dx: 8 }, 200, 485, 74, 4],
  ['roupi', { eyes: 'lazy', mouth: 'smirk', arms: ['point', 'rest'], legs: ['sit', 'sit'], tilt: -2, dx: 10 }, 225, 625, 6, 6],
];
const HELLO: Cast = [
  ['pave', { eyes: 'lazy', brows: 'up', mouth: 'smirk', arms: ['rest', 'wave'], legs: ['stand', 'stand'], tilt: 3 }, 225, 0, 6, 6, 'happy'],
  ['zeste', { eyes: 'happy', brows: 'up', mouth: 'grin', arms: ['up', 'wave'], legs: ['hop', 'hop'] }, 200, 140, 74, 4, 'celebrate'],
  ['bulle', { eyes: 'side', mouth: 'smile', arms: ['rest', 'wave'], legs: ['shy', 'shy'], tilt: -3 }, 260, 280, 10, 3, 'happy'],
  ['paillette', { eyes: 'stars', brows: 'up', mouth: 'grin', arms: ['wave', 'up'], legs: ['tiptoe', 'tiptoe'], tilt: -5 }, 200, 485, 74, 4, 'happy'],
  ['roupi', { eyes: 'lazy', mouth: 'smile', arms: ['wave', 'rest'], legs: ['sit', 'sit'], tilt: -2 }, 225, 625, 6, 6, 'happy'],
];
const W = 850;
const H = 270;

export const MascotBand = memo(function MascotBand({
  width,
  max = 460,
  hello,
  style,
}: {
  width?: number;
  max?: number;
  hello?: boolean;
  style?: StyleProp<ViewStyle>;
}) {
  const [measured, setMeasured] = useState(0);
  const w = width ?? Math.min(measured, max);
  const k = w / W;
  return (
    <View
      onLayout={width ? undefined : (e) => setMeasured(Math.round(e.nativeEvent.layout.width))}
      style={[width ? { width } : { alignSelf: 'stretch', alignItems: 'center' }, { height: (width ?? Math.min(measured || max, max)) * (H / W) }, style]}
      pointerEvents="none"
      aria-hidden
      importantForAccessibility="no-hide-descendants"
      accessibilityElementsHidden>
      {w > 0 && (
        <View style={{ width: w, height: H * k }}>
          {(hello ? HELLO : ACCUSE).map(([who, mood, size, left, bottom, z, motion]) => (
            <Mascot
              key={who}
              who={who}
              mood={mood}
              size={size * k}
              motion={motion ?? (mood === 'caught' ? 'caught' : 'accuse')}
              style={{ position: 'absolute', left: left * k, bottom: bottom * k, zIndex: z }}
            />
          ))}
        </View>
      )}
    </View>
  );
});
