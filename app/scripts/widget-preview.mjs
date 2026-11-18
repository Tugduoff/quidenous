// Off-device previews of the home-screen widget (docs/WIDGET.md): the real drawing (src/widget/widget.tsx) for sample
// states, both themes, small and medium, laid out as HTML. Android draws it with react-native-android-widget instead
// (LinearLayout + bitmaps), so this is close, not pixel-exact: check on a phone before trusting a detail.
//
//   bun scripts/widget-preview.mjs [out.html]      (from app/; default widget-preview.html)
//
// Then screenshot the page (any browser, 2-3x). `#small-dark-vote`-style ids frame each widget for a cropped shot.
import { writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

// The drawing imports the widget library, react-native and the secure store (theme.ts): stand-ins that only name things.
const prim = (name) => Object.assign(() => null, { __name__: name });
Bun.plugin({
  name: 'widget-preview-stubs',
  setup(build) {
    build.module('react-native-android-widget', () => ({
      loader: 'object',
      exports: {
        FlexWidget: prim('Flex'),
        TextWidget: prim('Text'),
        SvgWidget: prim('Svg'),
        ImageWidget: prim('Image'),
        OverlapWidget: prim('Overlap'),
        IconWidget: prim('Icon'),
        ListWidget: prim('List'),
      },
    }));
    build.module('react-native', () => ({ loader: 'object', exports: { Platform: { OS: 'android' }, useColorScheme: () => 'dark' } }));
    build.module('expo-secure-store', () => ({
      loader: 'object',
      exports: { getItemAsync: async () => null, setItemAsync: async () => {}, deleteItemAsync: async () => {} },
    }));
  },
});

const { QdnWidget } = await import('../src/widget/widget.tsx');

const font = {
  BricolageGrotesque_800ExtraBold: 'node_modules/@expo-google-fonts/bricolage-grotesque/800ExtraBold/BricolageGrotesque_800ExtraBold.ttf',
  Inter_600SemiBold: 'node_modules/@expo-google-fonts/inter/600SemiBold/Inter_600SemiBold.ttf',
  Inter_700Bold: 'node_modules/@expo-google-fonts/inter/700Bold/Inter_700Bold.ttf',
};

const px = (v) => (v === 'match_parent' ? '100%' : v === 'wrap_content' ? 'auto' : typeof v === 'number' ? `${v}px` : undefined);
const esc = (s) => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
const align = { 'flex-start': 'flex-start', center: 'center', 'flex-end': 'flex-end' };

function boxCss(st = {}) {
  const css = [];
  const put = (k, v) => v != null && v !== '' && css.push(`${k}:${v}`);
  put('width', px(st.width));
  put('height', px(st.height));
  if (st.flex) put('flex', `${st.flex} 1 0`), put('min-width', '0'), put('min-height', '0');
  for (const side of ['', 'Horizontal', 'Vertical', 'Top', 'Bottom', 'Left', 'Right'])
    for (const kind of ['padding', 'margin']) {
      const v = st[`${kind}${side}`];
      if (v == null) continue;
      if (side === 'Horizontal') put(`${kind}-left`, `${v}px`), put(`${kind}-right`, `${v}px`);
      else if (side === 'Vertical') put(`${kind}-top`, `${v}px`), put(`${kind}-bottom`, `${v}px`);
      else put(side ? `${kind}-${side.toLowerCase()}` : kind, `${v}px`);
    }
  put('background', st.backgroundColor);
  if (st.borderRadius != null) put('border-radius', `${st.borderRadius}px`);
  return css;
}

/** The element tree → HTML, components called down to the widget primitives (like the library's buildWidgetTree). */
function html(el) {
  if (el == null || el === false) return '';
  if (Array.isArray(el)) return el.map(html).join('');
  if (typeof el === 'string' || typeof el === 'number') return esc(el);
  let node = el;
  while (node && typeof node.type === 'function' && !node.type.__name__) node = node.type(node.props);
  if (node == null || node === false) return '';
  if (Array.isArray(node)) return node.map(html).join('');
  const { children, style = {}, ...p } = node.props;
  switch (node.type.__name__) {
    case 'Flex': {
      const row = style.flexDirection === 'row';
      const css = [...boxCss(style), 'display:flex', `flex-direction:${row ? 'row' : 'column'}`, 'box-sizing:border-box', 'overflow:hidden'];
      if (style.alignItems) css.push(`align-items:${align[style.alignItems]}`);
      if (style.justifyContent) css.push(`justify-content:${style.justifyContent}`);
      if (!style.alignItems) css.push('align-items:flex-start');
      const tap = p.clickActionData?.uri ? ` data-uri="${esc(p.clickActionData.uri)}"` : '';
      return `<div style="${css.join(';')}"${tap}>${html(children)}</div>`;
    }
    case 'Text': {
      const css = [...boxCss(style), `font-size:${style.fontSize ?? 12}px`, `color:${style.color ?? '#000'}`, `font-family:'${style.fontFamily ?? 'sans-serif'}'`];
      if (style.lineHeight) css.push(`line-height:${style.lineHeight}px`);
      if (p.maxLines === 1) css.push('white-space:nowrap', 'overflow:hidden', 'text-overflow:ellipsis', 'max-width:100%');
      else if (p.maxLines) css.push('display:-webkit-box', `-webkit-line-clamp:${p.maxLines}`, '-webkit-box-orient:vertical', 'overflow:hidden');
      return `<div style="${css.join(';')}">${esc(p.text)}</div>`;
    }
    case 'Svg':
      return `<img style="${[...boxCss(style), 'flex:none'].join(';')}" src="data:image/svg+xml;utf8,${encodeURIComponent(p.svg)}">`;
    default:
      return '';
  }
}

const q = {
  fr: 'Qui finirait par adopter un chat errant en vacances et le ramener dans sa valise ?',
  en: 'Who would adopt a stray cat on holiday and bring it home in their suitcase?',
};
const base = (lang, over) => ({
  kind: 'state',
  at: Date.now(),
  state: {
    state: 'vote',
    lang,
    room: { id: 3, name: lang === 'fr' ? 'Les Bolosses' : 'The Crew', kind: 'room' },
    round: { id: 42, number: 42, question: q[lang], kind: 'pick' },
    voted: false,
    votes: 3,
    needed: 6,
    result: null,
    unseen: 0,
    streak: { current: 12, today: false },
    next_open_at: null,
    ...over,
  },
});
const result = { id: 41, number: 41, question: 'Qui a le plus de chances de rater son avion pour une sieste ?', kind: 'pick' };
const states = [
  ['vote', base('fr', {})],
  ['wait', base('fr', { state: 'wait', voted: true, votes: 4, streak: { current: 12, today: true } })],
  ['result', base('fr', { state: 'result', result, unseen: 1, streak: { current: 12, today: true } })],
  ['idle', base('fr', { state: 'idle', round: null, next_open_at: new Date(new Date().setHours(21, 30, 0, 0)).toISOString() })],
  ['out', { kind: 'out' }],
  ['vote-en', base('en', {})],
];
const sizes = { small: [170, 170], medium: [350, 170] };

const cells = [];
for (const [size, [w, h]] of Object.entries(sizes))
  for (const mode of ['dark', 'light'])
    for (const [name, view] of states) {
      const tree = QdnWidget({ view, mode, width: w, height: h, lang: 'fr' });
      cells.push(
        `<figure id="${size}-${mode}-${name}" style="margin:0;width:${w}px;height:${h}px">${html(tree)}</figure>` +
          `<figcaption>${size} · ${mode} · ${name}</figcaption>`,
      );
    }

const faces = Object.entries(font)
  .map(([name, path]) => `@font-face{font-family:'${name}';src:url('file://${resolve(path)}')}`)
  .join('');
const page = `<!doctype html><meta charset="utf-8"><style>${faces}
body{margin:0;padding:24px;background:linear-gradient(135deg,#3a4a7a,#7a4a6a);font-family:sans-serif}
.grid{display:grid;grid-template-columns:repeat(6,max-content);gap:28px 22px;align-items:start}
.cell{display:flex;flex-direction:column;gap:6px} figcaption{color:#fff;opacity:.8;font-size:11px}
figure>div{box-shadow:0 4px 18px rgba(0,0,0,.25)}</style>
<div class="grid">${cells.map((c) => `<div class="cell">${c}</div>`).join('')}</div>`;
const out = resolve(process.argv[2] ?? 'widget-preview.html');
writeFileSync(out, page);
console.log(`wrote ${out} (${cells.length} widgets)`);
