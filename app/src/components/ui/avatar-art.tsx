import { memo } from 'react';
import Svg, { Circle, Ellipse, Path, Rect } from 'react-native-svg';

import { type Avatar, avatarShapes, parseAvatar, type Shape } from '@/lib/avatar';

function shape(s: Shape, i: number) {
  const paint = {
    fill: s.fill,
    opacity: s.op,
    ...(s.stroke && { stroke: s.stroke, strokeWidth: s.sw ?? 2.5, strokeLinecap: 'round' as const, strokeLinejoin: 'round' as const }),
  };
  switch (s.el) {
    case 'path':
      return <Path key={i} d={s.d} {...paint} />;
    case 'circle':
      return <Circle key={i} cx={s.cx} cy={s.cy} r={s.r} {...paint} />;
    case 'ellipse':
      return <Ellipse key={i} cx={s.cx} cy={s.cy} rx={s.rx} ry={s.ry} {...paint} />;
    case 'rect':
      return <Rect key={i} x={s.x} y={s.y} width={s.w} height={s.h} rx={s.rx} {...paint} />;
  }
}

/**
 * A drawn avatar, square: the parent clips it round (Avatar does). `viewBox` zooms in (the builder's close-ups).
 * Takes a spec string or an already parsed avatar; an invalid spec draws nothing.
 */
export const AvatarArt = memo(function AvatarArt({
  avatar,
  size,
  viewBox = '0 0 100 100',
}: {
  avatar: string | Avatar;
  size: number;
  viewBox?: string;
}) {
  const a = typeof avatar === 'string' ? parseAvatar(avatar) : avatar;
  if (!a) return null;
  return (
    <Svg width={size} height={size} viewBox={viewBox}>
      {avatarShapes(a).map(shape)}
    </Svg>
  );
});
