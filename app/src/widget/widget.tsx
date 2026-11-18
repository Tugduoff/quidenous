'use no memo';
import { FlexWidget, SvgWidget, TextWidget } from 'react-native-android-widget';

import { tIn, type Lang } from '@/lib/i18n-core';
import { ACCENTS, COLORS, FONT, themes } from '@/lib/tokens';

import { ART, type ArtKey } from '@/widget/art';
import type { WidgetView } from '@/widget/state';

type Mode = 'light' | 'dark';

type Hex = `#${string}`;
const palette = (mode: Mode) => {
  const th = themes[mode] as unknown as Record<'bg' | 'card' | 'card2' | 'txt' | 'mut' | 'tag' | 'accInk', Hex>;
  return {
    bg: mode === 'dark' ? th.bg : th.card,
    txt: th.txt,
    mut: th.mut,
    chip: th.card2,
    lime: ACCENTS.lime.acc,
    onLime: themes.dark.bg as Hex,
    pink: th.tag,
    brand: th.accInk,
  } satisfies Record<string, Hex>;
};
type Palette = ReturnType<typeof palette>;

const flame = (color: string) =>
  `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24" fill="none" stroke="${color}" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3q1 4 4 6.5t3 5.5a1 1 0 0 1-14 0 5 5 0 0 1 1-3 1 1 0 0 0 5 0c0-2-1.5-3-1.5-5q0-2 2.5-4"/></svg>`;

const art = (key: ArtKey, mode: Mode) =>
  mode === 'dark' ? ART[key].replace(/fill="#000000" opacity="0\.1(6)?"/, (_, hi) => `fill="${COLORS.gloss}" opacity="${hi ? 0.07 : 0.05}"`) : ART[key];

type Face = {
  title: string;
  status: string;
  sub: string | null;
  chip: 'lime' | 'pink' | 'plain';
  mascot: ArtKey;
  uri: string;
  room: string | null;
  streak: { n: number; lit: boolean } | null;
};

const SCHEME = 'quidenous://';

function clock(iso: string, lang: Lang) {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return null;
  try {
    return new Intl.DateTimeFormat(lang === 'fr' ? 'fr-FR' : 'en-GB', { hour: '2-digit', minute: '2-digit' }).format(d);
  } catch {
    return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
  }
}

export function faceOf(view: WidgetView, fallbackLang: Lang): Face {
  if (view.kind !== 'state' || view.state.state === 'no_room') {
    const lang = view.kind === 'state' ? view.state.lang : fallbackLang;
    const t = (k: Parameters<typeof tIn>[1]) => tIn(lang, k);
    return {
      title: view.kind === 'loading' ? t('widget.loading') : t('widget.open'),
      status: view.kind === 'out' ? t('widget.signedOut') : view.kind === 'loading' ? '' : t('widget.noRoom'),
      sub: null,
      chip: 'plain',
      mascot: 'hello',
      uri: SCHEME,
      room: null,
      streak: null,
    };
  }
  const s = view.state;
  const t = (k: Parameters<typeof tIn>[1], p?: Record<string, string | number>) => tIn(s.lang, k, p);
  const roomId = s.room?.id;
  const here = roomId ? `?room=${roomId}` : '';
  const base = { room: s.room?.name ?? null, streak: s.streak && s.streak.current > 0 ? { n: s.streak.current, lit: s.streak.today } : null };
  switch (s.state) {
    case 'result':
      return {
        ...base,
        title: s.result?.question ?? '',
        status: s.unseen > 1 ? t('widget.results', { count: s.unseen }) : t('widget.result'),
        sub: s.round && !s.voted ? t('widget.nextToo') : null,
        chip: 'pink',
        mascot: 'result',
        uri: `${SCHEME}round/${s.result?.id}${here}`,
      };
    case 'vote':
      return { ...base, title: s.round?.question ?? '', status: t('widget.vote'), sub: null, chip: 'lime', mascot: 'vote', uri: `${SCHEME}${here}` };
    case 'wait': {
      const left = Math.max(s.needed - s.votes, 1);
      return { ...base, title: s.round?.question ?? '', status: t('widget.wait', { count: left }), sub: null, chip: 'plain', mascot: 'wait', uri: `${SCHEME}${here}` };
    }
    default: {
      const at = s.next_open_at ? clock(s.next_open_at, s.lang) : null;
      return { ...base, title: t('widget.idle'), status: at ? t('widget.idleAt', { time: at }) : '', sub: null, chip: 'plain', mascot: 'idle', uri: `${SCHEME}${here}` };
    }
  }
}

function Chip({ face, c, size }: { face: Face; c: Palette; size: number }) {
  if (!face.status) return null;
  const bg = face.chip === 'lime' ? c.lime : face.chip === 'pink' ? c.pink : c.chip;
  const fg: Hex = face.chip === 'lime' ? c.onLime : face.chip === 'pink' ? (COLORS.onStrong as Hex) : c.txt;
  return (
    <FlexWidget style={{ backgroundColor: bg, borderRadius: 999, paddingHorizontal: 10, paddingVertical: 4 }}>
      <TextWidget text={face.status} maxLines={1} truncate="END" style={{ fontSize: size, fontFamily: FONT.semibold, color: fg }} />
    </FlexWidget>
  );
}

function Streak({ face, c, size }: { face: Face; c: Palette; size: number }) {
  if (!face.streak) return null;
  const color = face.streak.lit ? c.brand : c.mut;
  return (
    <FlexWidget style={{ flexDirection: 'row', alignItems: 'center' }}>
      <SvgWidget svg={flame(color)} style={{ width: size + 2, height: size + 2 }} />
      <TextWidget text={String(face.streak.n)} style={{ fontSize: size, fontFamily: FONT.semibold, color, marginLeft: 2 }} />
    </FlexWidget>
  );
}

function Header({ face, c, small }: { face: Face; c: Palette; small: boolean }) {
  return (
    <FlexWidget style={{ flexDirection: 'row', alignItems: 'center', width: 'match_parent' }}>
      <TextWidget text="qui de nous ?" maxLines={1} style={{ fontSize: small ? 12 : 13, fontFamily: FONT.display, color: c.brand }} />
      {!small && face.room ? (
        <TextWidget
          text={`  ·  ${face.room}`}
          maxLines={1}
          truncate="END"
          style={{ fontSize: 12, fontFamily: FONT.semibold, color: c.mut }}
        />
      ) : null}
      <FlexWidget style={{ flex: 1 }} />
      <Streak face={face} c={c} size={small ? 11 : 12} />
    </FlexWidget>
  );
}

export function QdnWidget({ view, mode, width, height, lang }: { view: WidgetView; mode: Mode; width: number; height: number; lang: Lang }) {
  const c = palette(mode);
  const face = faceOf(view, lang);
  const small = width < 250;
  const tap = { clickAction: 'OPEN_URI', clickActionData: { uri: face.uri } } as const;
  const root = {
    width: 'match_parent',
    height: 'match_parent',
    backgroundColor: c.bg,
    borderRadius: 24,
    padding: small ? 12 : 14,
  } as const;

  if (small) {
    const mascot = Math.min(56, Math.max(40, Math.round(height * 0.32)));
    return (
      <FlexWidget {...tap} style={{ ...root, flexDirection: 'column', justifyContent: 'space-between' }}>
        <Header face={face} c={c} small />
        <TextWidget
          text={face.title}
          maxLines={height < 150 ? 2 : 3}
          truncate="END"
          style={{ fontSize: 14, fontFamily: FONT.bold, color: c.txt, lineHeight: 18, marginTop: 6 }}
        />
        <FlexWidget style={{ flexDirection: 'row', alignItems: 'flex-end', width: 'match_parent' }}>
          <FlexWidget style={{ flex: 1, paddingBottom: 2 }}>
            <TextWidget
              text={face.status}
              maxLines={2}
              truncate="END"
              style={{ fontSize: 12, fontFamily: FONT.semibold, color: face.chip === 'lime' ? c.brand : face.chip === 'pink' ? c.pink : c.mut }}
            />
          </FlexWidget>
          <SvgWidget svg={art(face.mascot, mode)} style={{ width: mascot, height: mascot }} />
        </FlexWidget>
      </FlexWidget>
    );
  }

  const mascot = Math.min(110, Math.max(72, Math.round(height * 0.72)));
  return (
    <FlexWidget {...tap} style={{ ...root, flexDirection: 'row', alignItems: 'center' }}>
      <FlexWidget style={{ flex: 1, height: 'match_parent', flexDirection: 'column', justifyContent: 'space-between' }}>
        <Header face={face} c={c} small={false} />
        <TextWidget
          text={face.title}
          maxLines={height < 150 ? 3 : 4}
          truncate="END"
          style={{ fontSize: 16, fontFamily: FONT.bold, color: c.txt, lineHeight: 21, marginTop: 6, marginBottom: 6 }}
        />
        <FlexWidget style={{ flexDirection: 'column' }}>
          <Chip face={face} c={c} size={12} />
          {face.sub ? (
            <TextWidget text={face.sub} maxLines={1} truncate="END" style={{ fontSize: 11, fontFamily: FONT.semibold, color: c.mut, marginTop: 4 }} />
          ) : null}
        </FlexWidget>
      </FlexWidget>
      <SvgWidget svg={art(face.mascot, mode)} style={{ width: mascot, height: mascot, marginLeft: 8 }} />
    </FlexWidget>
  );
}

export function renderBoth(view: WidgetView, width: number, height: number, lang: Lang) {
  return {
    light: <QdnWidget view={view} mode="light" width={width} height={height} lang={lang} />,
    dark: <QdnWidget view={view} mode="dark" width={width} height={height} lang={lang} />,
  };
}
