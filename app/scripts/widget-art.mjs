import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

const src = resolve('src/lib/utils/mascot.ts');
const { compose, GROUND } = await import(src);

const n = (v) => (v == null ? '' : String(Math.round(v * 100) / 100));
const attr = (k, v) => (v == null || v === '' ? '' : ` ${k}="${v}"`);

function shape(s) {
  const paint =
    attr('fill', s.fill ?? 'none') +
    attr('opacity', s.op) +
    (s.stroke ? attr('stroke', s.stroke) + attr('stroke-width', s.sw) + ' stroke-linecap="round" stroke-linejoin="round"' : '');
  switch (s.el) {
    case 'path':
      return `<path d="${s.d}"${paint}/>`;
    case 'circle':
      return `<circle cx="${n(s.cx)}" cy="${n(s.cy)}" r="${n(s.r)}"${paint}/>`;
    case 'ellipse':
      return `<ellipse cx="${n(s.cx)}" cy="${n(s.cy)}" rx="${n(s.rx)}" ry="${n(s.ry)}"${paint}/>`;
    case 'rect':
      return `<rect x="${n(s.x)}" y="${n(s.y)}" width="${n(s.w)}" height="${n(s.h)}"${attr('rx', s.rx)}${paint}/>`;
  }
}
const g = (grp) => (grp && grp.shapes.length ? `<g${attr('transform', grp.tr)}>${grp.shapes.map(shape).join('')}</g>` : '');

function svg(c, flip = false) {
  const f = c.fill;
  const fill = f.gloss
    ? `<path d="${f.d}" fill="${f.shade}"/><g clip-path="url(#b)"><path d="${f.d}" fill="${f.color}" transform="translate(-10 -9)"/></g>` +
      `<path d="${f.gloss}" fill="none" stroke="#FFFFFF" stroke-width="6" stroke-linecap="round" opacity="0.75"/>` +
      `<path d="${f.d}" fill="none" stroke="${f.outline}" stroke-width="${f.sw}" stroke-linejoin="round"/>`
    : `<path d="${f.d}" fill="${f.color}" stroke="${f.outline}" stroke-width="${f.sw}" stroke-linejoin="round"/>`;
  const sh = c.shadow;
  const inBody = c.prop && c.prop.at !== 'abs' ? g(c.prop) : '';
  const abs = c.prop?.at === 'abs' ? g(c.prop) : '';
  const body =
    `<ellipse cx="${n(sh.cx)}" cy="${n(GROUND + 1)}" rx="${n(sh.rx)}" ry="${n(sh.ry)}" fill="#000000" opacity="${sh.air ? 0.1 : 0.16}"/>` +
    `<g transform="${c.body}">${c.back.map(g).join('')}${fill}${c.over.map(g).join('')}${g(c.under)}${g(c.eyes)}${g(c.face)}${c.front.map(g).join('')}${inBody}</g>${abs}`;
  return (
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 200" width="200" height="200">` +
    (f.gloss ? `<defs><clipPath id="b"><path d="${f.d}"/></clipPath></defs>` : '') +
    (flip ? `<g transform="translate(200 0) scale(-1 1)">${body}</g>` : body) +
    `</svg>`
  );
}

const art = {
  wait: svg(compose('roupi', 'sleepy')),
  vote: svg(
    compose('zeste', { eyes: 'open', brows: 'up', mouth: 'grin', arms: ['rest', 'point'], legs: ['wide', 'wide'], tilt: 4 }),
    true,
  ),
  result: svg(compose('paillette', 'celebrate')),
  idle: svg(compose('pave', 'think')),
  hello: svg(compose('zeste', 'happy')),
};

const out = resolve('src/widget/art.ts');
const generated =
  `// @generated\n` +
  `export const ART = ${JSON.stringify(art, null, 2)} as const;\n\nexport type ArtKey = keyof typeof ART;\n`;

if (process.argv.includes('--check')) {
  if (readFileSync(out, 'utf8') !== generated) {
    console.error('src/widget/art.ts is out of date or was edited by hand: run npx tsx scripts/widget-art.mjs');
    process.exit(1);
  }
  console.log('widget art up to date');
} else {
  writeFileSync(out, generated);
  console.log(`wrote ${out}: ${Object.entries(art).map(([k, v]) => `${k} ${(v.length / 1024).toFixed(1)} KB`).join(', ')}`);
}
