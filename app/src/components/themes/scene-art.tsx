import { memo, useId } from 'react';
import Svg, { Circle, Defs, LinearGradient, Path, RadialGradient, Rect, Stop } from 'react-native-svg';

import { type ArtGrad, type ArtShape, scene, SCENE_H } from '@/lib/utils/room-art';
import type { CuratedId } from '@/lib/utils/room-theme';

function piece(s: ArtShape, i: number, uid: string) {
  const paint = {
    fill: s.grad ? `url(#${uid}${s.grad})` : (s.fill ?? 'none'),
    opacity: s.op,
    transform: s.tr,
    ...(s.stroke && { stroke: s.stroke, strokeWidth: s.sw, strokeLinecap: s.flat ? ('butt' as const) : ('round' as const), strokeLinejoin: 'round' as const }),
  };
  switch (s.el) {
    case 'path':
      return <Path key={i} d={s.d} {...paint} />;
    case 'circle':
      return <Circle key={i} cx={s.cx} cy={s.cy} r={s.r} {...paint} />;
    case 'rect':
      return <Rect key={i} x={s.x} y={s.y} width={s.w} height={s.h} rx={s.rx} {...paint} />;
  }
}

function gradient(g: ArtGrad, uid: string) {
  const stops = g.stops.map(([offset, color, alpha], i) => <Stop key={i} offset={offset} stopColor={color} stopOpacity={alpha} />);
  return g.kind === 'linear' ? (
    <LinearGradient key={g.id} id={uid + g.id} gradientUnits="userSpaceOnUse" x1={g.x1} y1={g.y1} x2={g.x2} y2={g.y2}>
      {stops}
    </LinearGradient>
  ) : (
    <RadialGradient key={g.id} id={uid + g.id} gradientUnits="userSpaceOnUse" cx={g.cx} cy={g.cy} r={g.r}>
      {stops}
    </RadialGradient>
  );
}

export const SceneArt = memo(function SceneArt({ id, dark, width, height = SCENE_H }: { id: CuratedId; dark: boolean; width: number; height?: number }) {
  const uid = useId().replace(/[^a-zA-Z0-9]/g, '');
  const sc = scene(id, dark, width / (height / SCENE_H));
  return (
    <Svg width={width} height={height} viewBox={`0 0 ${sc.w} ${sc.h}`} preserveAspectRatio="xMaxYMin slice">
      <Defs>{sc.grads.map((g) => gradient(g, uid))}</Defs>
      {sc.shapes.map((s, i) => piece(s, i, uid))}
    </Svg>
  );
});
