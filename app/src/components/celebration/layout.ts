import type { ViewStyle } from 'react-native';

/** Pinned to the stage's top-left corner (moved by transforms). */
export const ABS: ViewStyle = { position: 'absolute', left: 0, top: 0 };

/** Positioned box centred on (x, y). */
export const centred = (x: number, y: number, w: number, h: number): ViewStyle => ({
  position: 'absolute',
  left: x - w / 2,
  top: y - h / 2,
  width: w,
  height: h,
});
