const tenth = (v: number) => Math.round(v * 10) / 10;

const PAD_SHARE = 0.09;
const CREST_SHARE = 0.13;
const CREST_TO_FLOOR_PX = 6;
const BANK_DEPTH_SHARE = 0.085;

export type Snowdrift = {
  width: number;
  height: number;
  pad: number;
  floor: number;
  hill: string;
  crest: string;
  bank: string;
  edge: string;
};

export function snowdrift(bandWidth: number): Snowdrift {
  const w = bandWidth;
  const pad = Math.round(w * PAD_SHARE);
  const width = w + 2 * pad;
  const floor = Math.round(w * CREST_SHARE + CREST_TO_FLOOR_PX);
  const height = Math.round(floor + w * BANK_DEPTH_SHARE);
  const x = (k: number) => tenth(pad + w * k);
  const y = (k: number) => tenth(floor - w * k);
  const ridge = `M${x(0.04)} ${y(0)}C${x(0.1)} ${y(0.06)} ${x(0.16)} ${y(0.106)} ${x(0.27)} ${y(0.108)}C${x(0.38)} ${y(0.11)} ${x(0.42)} ${y(0.094)} ${x(0.5)} ${y(0.094)}C${x(0.58)} ${y(0.094)} ${x(0.62)} ${y(0.11)} ${x(0.73)} ${y(0.108)}C${x(0.84)} ${y(0.106)} ${x(0.9)} ${y(0.06)} ${x(0.96)} ${y(0)}`;
  const lip = `M${tenth(0)} ${y(-0.012)}C${x(-0.02)} ${y(0.026)} ${x(0.1)} ${y(0.04)} ${x(0.24)} ${y(0.03)}C${x(0.36)} ${y(0.022)} ${x(0.42)} ${y(0.036)} ${x(0.54)} ${y(0.036)}C${x(0.68)} ${y(0.036)} ${x(0.74)} ${y(0.022)} ${x(0.84)} ${y(0.03)}C${x(0.95)} ${y(0.04)} ${x(1.03)} ${y(0.024)} ${tenth(width)} ${y(-0.012)}`;
  return {
    width,
    height,
    pad,
    floor,
    hill: `${ridge}Z`,
    crest: ridge,
    bank: `${lip}V${height}H0Z`,
    edge: lip,
  };
}
