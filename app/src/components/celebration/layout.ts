import type { ViewStyle } from 'react-native';

export const ABS: ViewStyle = { position: 'absolute', left: 0, top: 0 };

export const centred = (x: number, y: number, w: number, h: number): ViewStyle => ({
  position: 'absolute',
  left: x - w / 2,
  top: y - h / 2,
  width: w,
  height: h,
});
