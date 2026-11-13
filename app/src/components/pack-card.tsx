import { type Href, router } from 'expo-router';
import { Check, ChevronLeft, Download, Heart, Star } from 'lucide-react-native';
import type { ReactNode } from 'react';
import { Text, View } from 'react-native';

import { Card, PressableScale } from '@/components/ui';
import { formatNumber, useT } from '@/lib/i18n';
import type { PackItem } from '@/lib/packs';
import { useTheme } from '@/lib/theme';

/** A small tag next to a pack's name: its language ("EN") when not the room's, "16+". */
export function MiniTag({ label, tone = 'mut' }: { label: string; tone?: 'mut' | 'hot' }) {
  const t = useTheme();
  return (
    <View
      style={{
        paddingHorizontal: 6,
        height: 18,
        borderRadius: 6,
        justifyContent: 'center',
        backgroundColor: tone === 'hot' ? t.tag : t.card2,
      }}>
      <Text style={{ fontFamily: t.font.bold, fontSize: 10, letterSpacing: 0.5, color: tone === 'hot' ? '#fff' : t.mut }}>{label}</Text>
    </View>
  );
}

/** A pack's status for its author: "En revue" (grey) or "Retiré" (pink). Nothing when public. */
export function StatusBadge({ status }: { status: PackItem['status'] }) {
  const t = useTheme();
  const tr = useT();
  if (status !== 'review' && status !== 'removed') return null;
  const removed = status === 'removed';
  return (
    <View
      style={{
        paddingHorizontal: 8,
        height: 22,
        borderRadius: 11,
        justifyContent: 'center',
        backgroundColor: removed ? t.tag : t.card2,
      }}>
      <Text style={{ fontFamily: t.font.bold, fontSize: 11, color: removed ? '#fff' : t.mut }}>{tr(`packs.status.${status}`)}</Text>
    </View>
  );
}

/** A count with its icon (likes, installs). */
export function Stat({ icon: Icon, value, on, label }: { icon: typeof Heart; value: number; on?: boolean; label: string }) {
  const t = useTheme();
  return (
    <View accessible accessibilityLabel={label} style={{ flexDirection: 'row', alignItems: 'center', gap: 4 }}>
      <Icon size={14} color={on ? t.tag : t.mut} fill={on ? t.tag : 'transparent'} />
      <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>{formatNumber(value)}</Text>
    </View>
  );
}

/**
 * One pack in a list: name with its marks, author and size, description, likes and installs, "Installé" when in this room.
 * `roomLang`: a pack in another language gets a tag. `withStatus`: my packs, with their moderation status.
 */
export function PackCard({ pack: p, roomLang, withStatus }: { pack: PackItem; roomLang?: string; withStatus?: boolean }) {
  const t = useTheme();
  const tr = useT();
  return (
    <PressableScale scale={0.98} accessibilityRole="button" accessibilityLabel={p.name} onPress={() => router.push(`/pack/${p.id}`)}>
      <Card style={{ padding: 16, gap: 6 }}>
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
          <Text numberOfLines={1} style={{ flexShrink: 1, fontFamily: t.font.bold, fontSize: 17, color: t.txt }}>
            {p.name}
          </Text>
          {p.featured && <Star size={14} color={t.accInk} fill={t.accInk} accessibilityLabel={tr('packs.featured')} />}
          <View style={{ flex: 1 }} />
          {!!p.language && !!roomLang && p.language !== roomLang && <MiniTag label={p.language.toUpperCase()} />}
          {p.spicy && <MiniTag label={tr('packs.spicy')} tone="hot" />}
          {withStatus && <StatusBadge status={p.status} />}
        </View>
        <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>
          {tr('packs.byline', { count: p.count, author: p.author })}
        </Text>
        {!!p.description && (
          <Text numberOfLines={2} style={{ fontFamily: t.font.regular, fontSize: 14, lineHeight: 19, color: t.txt, opacity: 0.85 }}>
            {p.description}
          </Text>
        )}
        {withStatus && p.status === 'removed' && !!p.removed_reason && (
          <Text numberOfLines={2} style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.tag }}>
            {tr('packs.removedReason', { reason: p.removed_reason })}
          </Text>
        )}
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 14, marginTop: 2 }}>
          <Stat icon={Heart} value={p.likes} on={p.liked} label={tr('packs.likes', { count: p.likes })} />
          <Stat icon={Download} value={p.installs} label={tr('packs.installs', { count: p.installs })} />
          <View style={{ flex: 1 }} />
          {p.installed && (
            <View style={{ flexDirection: 'row', alignItems: 'center', gap: 4 }}>
              <Check size={14} color={t.accInk} />
              <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.accInk }}>{tr('packs.installedHere')}</Text>
            </View>
          )}
        </View>
      </Card>
    </PressableScale>
  );
}

/** A round icon button for page headers. */
export function IconButton({
  icon: Icon,
  label,
  onPress,
  on,
  disabled,
}: {
  icon: typeof Heart;
  label: string;
  onPress: () => void;
  on?: boolean;
  disabled?: boolean;
}) {
  const t = useTheme();
  return (
    <PressableScale
      accessibilityRole="button"
      accessibilityLabel={label}
      accessibilityState={{ disabled: !!disabled, selected: on }}
      disabled={disabled}
      onPress={onPress}
      hitSlop={4}
      style={{
        width: 40,
        height: 40,
        borderRadius: 20,
        alignItems: 'center',
        justifyContent: 'center',
        backgroundColor: on ? t.acc : t.card,
        opacity: disabled ? 0.5 : 1,
      }}>
      <Icon size={19} color={on ? t.accTxt : t.txt} />
    </PressableScale>
  );
}

/** Back on the left (to `fallback` when there is nothing to go back to), icon buttons on the right. */
export function PageHeader({ fallback, children }: { fallback: Href; children?: ReactNode }) {
  const tr = useT();
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
      <IconButton icon={ChevronLeft} label={tr('common.back')} onPress={() => (router.canGoBack() ? router.back() : router.replace(fallback))} />
      <View style={{ flex: 1 }} />
      {children}
    </View>
  );
}

/** A category filter (or the 16+ switch, `hot`). */
export function Chip({
  icon: Icon,
  label,
  on,
  hot,
  disabled,
  onPress,
}: {
  icon: typeof Heart;
  label: string;
  on: boolean;
  hot?: boolean;
  disabled?: boolean;
  onPress: () => void;
}) {
  const t = useTheme();
  const bg = on ? (hot ? t.tag : t.acc) : t.card;
  const fg = on ? (hot ? '#fff' : t.accTxt) : t.txt;
  return (
    <PressableScale
      accessibilityRole={hot ? 'switch' : 'button'}
      accessibilityState={hot ? { checked: on } : { selected: on }}
      accessibilityLabel={label}
      disabled={disabled}
      onPress={onPress}
      style={{
        flexDirection: 'row',
        alignItems: 'center',
        gap: 6,
        height: 34,
        paddingHorizontal: 12,
        borderRadius: t.radius.pill,
        backgroundColor: bg,
        opacity: disabled ? 0.4 : 1,
      }}>
      <Icon size={15} color={on ? fg : t.mut} />
      <Text numberOfLines={1} style={{ fontFamily: t.font.semibold, fontSize: 13, color: fg }}>
        {label}
      </Text>
    </PressableScale>
  );
}
