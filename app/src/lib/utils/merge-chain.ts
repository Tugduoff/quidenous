/** The 2048 party's tiles, from the first 2 to the final 2048. */
export const TILE_VALUES = [2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048] as const;

const FIRST_TILE_AT = 300;
const FOREVER = 1e9;

/**
 * When each merge lands (ms): the chain speeds up, then the last two 1024 take their time (`last`),
 * and the 2048 slams down after (`slam`).
 */
export function mergeChain() {
  const merges: number[] = [];
  let at = 500;
  for (let i = 0; i < 9; i++) {
    at += Math.max(150, 330 - i * 26);
    merges.push(at);
  }
  const last = merges[8] + 650;
  return { merges, last, slam: last + 330 };
}

type Chain = ReturnType<typeof mergeChain>;

/** The tile sitting in the merge cell: each value from the moment it is born until the next merge replaces it. */
export function mergeCellTiles({ merges, last }: Chain) {
  return TILE_VALUES.map((v, i) => ({
    v,
    from: i === 0 ? FIRST_TILE_AT : i <= 9 ? merges[i - 1] : last,
    to: i < 9 ? merges[i] : i === 9 ? last : FOREVER,
  }));
}

/**
 * The twin of each tile: pops in on a neighbour (`spawn`), slides into the merge cell (`start` → `end`).
 * `source` is which neighbour it comes from (0..3, in turn; the final 1024 comes from neighbour 0).
 */
export function twinTiles({ merges, last }: Chain) {
  return TILE_VALUES.slice(0, 10).map((v, i) => {
    const end = i < 9 ? merges[i] : last;
    const slide = i < 9 ? 110 : 260;
    return { v, spawn: end - slide - 170, start: end - slide, end, source: i < 9 ? i % 4 : 0 };
  });
}
