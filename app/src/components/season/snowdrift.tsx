import { memo, useId } from 'react';
import { View } from 'react-native';
import Svg, { Defs, LinearGradient, Path, Stop } from 'react-native-svg';
import { BAND_ASPECT } from '@/components/mascot/band';
import { useScheme } from '@/lib/theme';
import { SEASON } from '@/lib/tokens';
import { snowdrift } from '@/lib/utils/snowdrift';

const RIM_WIDTH = 3;
const BANK_SOLID_UNTIL = 0.25;
const OVER_THE_FRONT_ROW = 9;

export const Snowdrift = memo(function Snowdrift({ width, part }: { width: number; part: 'hill' | 'bank' }) {
  const tone = SEASON.drift[useScheme()];
  const gradient = `sd${useId().replace(/[^a-zA-Z0-9]/g, '')}`;
  const d = snowdrift(width);
  const top = width * BAND_ASPECT - d.floor;
  return (
    <View
      pointerEvents="none"
      style={{ position: 'absolute', left: -d.pad, top, width: d.width, height: d.height, zIndex: part === 'bank' ? OVER_THE_FRONT_ROW : 0 }}>
      <Svg width={d.width} height={d.height} viewBox={`0 0 ${d.width} ${d.height}`}>
        {part === 'hill' ? (
          <>
            <Path d={d.hill} fill={tone.hill} />
            <Path d={d.crest} fill="none" stroke={tone.crest} strokeWidth={RIM_WIDTH} strokeLinecap="round" />
          </>
        ) : (
          <>
            <Defs>
              <LinearGradient id={gradient} x1="0" y1="0" x2="0" y2="1">
                <Stop offset={BANK_SOLID_UNTIL} stopColor={tone.bank} stopOpacity={1} />
                <Stop offset={1} stopColor={tone.bank} stopOpacity={0} />
              </LinearGradient>
            </Defs>
            <Path d={d.bank} fill={`url(#${gradient})`} />
            <Path d={d.edge} fill="none" stroke={tone.edge} strokeWidth={RIM_WIDTH} strokeLinecap="round" />
          </>
        )}
      </Svg>
    </View>
  );
});
