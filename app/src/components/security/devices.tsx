import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Fingerprint, Globe, Lock, LogOut, ScanFace, Smartphone, X } from 'lucide-react-native';
import { useEffect, useState } from 'react';
import { Pressable, Text } from 'react-native';

import { ErrorText, Pill, Row, Section, TextLink } from '@/components/auth-ui';
import { BrandIcon } from '@/components/brand-icons';
import { askConfirm, Toggle } from '@/components/ui';
import {
  accountKey,
  auth,
  authError,
  isLinked,
  PROVIDER_NAMES,
  useAccount,
  useFeatures,
  type Account,
  type DeviceSession,
  type Provider,
} from '@/lib/auth';
import { t as translate, useT } from '@/lib/i18n';
import { lockMethod, lockSupported, setLock, useLock } from '@/lib/lock';
import { useSession } from '@/lib/session';
import { oauth } from '@/lib/sign-in';
import { useTheme } from '@/lib/theme';
import { ago } from '@/lib/time';

/** Google / Discord: link (the web leaves for the provider and comes back on /oauth), unlink. */
export function ProvidersSection({ account }: { account: Account }) {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const { providers } = useFeatures();
  const [busy, setBusy] = useState<Provider | null>(null);
  const [error, setError] = useState<string | null>(null);
  if (!providers.length) return null;

  const run = async (p: Provider, f: () => Promise<unknown>) => {
    setBusy(p);
    setError(null);
    try {
      await f();
      qc.invalidateQueries({ queryKey: accountKey });
    } catch (e) {
      setError(authError(e));
    } finally {
      setBusy(null);
    }
  };
  const link = (p: Provider) =>
    run(p, async () => {
      const r = await oauth(p, true);
      if (r && !isLinked(r)) throw new Error('not linked');
    });
  const unlink = async (p: Provider) => {
    if (await askConfirm(tr('security.providers.unlinkConfirm', { provider: PROVIDER_NAMES[p] }), undefined, tr('security.providers.unlink')))
      run(p, () => auth.unlink(p));
  };

  return (
    <Section title={tr('security.providers.title')}>
      {providers.map((p) => {
        const linked = account.providers.includes(p);
        return (
          <Row
            key={p}
            icon={<BrandIcon provider={p} />}
            title={PROVIDER_NAMES[p]}
            right={
              linked ? (
                <Pill tone="danger" onPress={() => unlink(p)} loading={busy === p} disabled={!!busy}>
                  {tr('security.providers.unlink')}
                </Pill>
              ) : (
                <Pill onPress={() => link(p)} loading={busy === p} disabled={!!busy}>
                  {tr('security.providers.link')}
                </Pill>
              )
            }
          />
        );
      })}
      <ErrorText>{error}</ErrorText>
    </Section>
  );
}

/** The server stores the label as sent (percent-encoded, see lib/device.ts): shown decoded. */
function label(s: DeviceSession) {
  if (!s.device) return translate('security.devices.unknown');
  try {
    return decodeURIComponent(s.device);
  } catch {
    return s.device;
  }
}

/** Where I'm signed in: this device first, sign one out, or every other one. */
export function SessionsSection() {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const { data: sessions } = useQuery({ queryKey: ['sessions'], queryFn: auth.sessions });
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const refresh = () => qc.invalidateQueries({ queryKey: ['sessions'] });
  const list = [...(sessions ?? [])].sort((a, b) => Number(b.current) - Number(a.current));
  const others = list.filter((s) => !s.current).length;

  const revoke = async (s: DeviceSession) => {
    if (!(await askConfirm(tr('security.devices.confirmOne'), label(s), tr('security.devices.signOut')))) return;
    setError(null);
    await auth.revoke(s.id).catch((e) => setError(authError(e)));
    refresh();
  };
  const revokeOthers = async () => {
    if (!(await askConfirm(tr('security.devices.confirmOthers'), undefined, tr('security.devices.signOut')))) return;
    setBusy(true);
    setError(null);
    await auth.revokeOthers().catch((e) => setError(authError(e)));
    setBusy(false);
    refresh();
  };

  if (!sessions) return null;
  return (
    <Section title={tr('security.devices.title')}>
      {list.map((s) => {
        const Icon = label(s).startsWith('Web') ? Globe : Smartphone;
        return (
          <Row
            key={s.id}
            icon={<Icon size={20} color={s.current ? t.accInk : t.mut} />}
            title={label(s)}
            sub={s.current ? tr('security.devices.current') : tr('security.devices.active', { when: ago(s.last_used_at ?? s.created_at) })}
            right={
              !s.current && (
                <Pressable
                  accessibilityRole="button"
                  accessibilityLabel={tr('security.devices.signOutA11y', { device: label(s) })}
                  onPress={() => revoke(s)}
                  hitSlop={8}
                  style={{ width: 36, height: 36, borderRadius: 18, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card2 }}>
                  <X size={16} color={t.mut} />
                </Pressable>
              )
            }
          />
        );
      })}
      <ErrorText>{error}</ErrorText>
      {others > 1 && (
        <TextLink onPress={revokeOthers} disabled={busy}>
          {tr('security.revokeOthers')}
        </TextLink>
      )}
    </Section>
  );
}

/** Face ID / fingerprint when coming back to the app (phones with the native module only). */
export function LockSection() {
  const t = useTheme();
  const tr = useT();
  const { enabled } = useLock();
  const [error, setError] = useState<string | null>(null);
  // Say what it actually asks for: "Face ID" means nothing on a phone with a fingerprint reader.
  const [method, setMethod] = useState<'face' | 'fingerprint' | 'code'>('code');
  useEffect(() => {
    lockMethod().then(setMethod);
  }, []);
  if (!lockSupported) return null;
  const label = tr(`security.lock.${method}`);
  const Icon = method === 'face' ? ScanFace : method === 'fingerprint' ? Fingerprint : Lock;
  const toggle = async (on: boolean) => {
    setError(null);
    const r = await setLock(on).catch(() => 'failed' as const);
    if (r === 'unavailable') setError(tr('security.lock.unavailable'));
  };
  return (
    <Section title={tr('security.lock.title')}>
      <Row
        icon={<Icon size={20} color={enabled ? t.accInk : t.mut} />}
        title={label}
        right={<Toggle label="" a11y={tr('security.lock.a11y', { method: label.toLowerCase() })} value={enabled} onChange={toggle} />}
      />
      <ErrorText>{error}</ErrorText>
    </Section>
  );
}

/** Sign out, after a warning when the account has no way back in (legacy, token only). */
export function useSignOut() {
  const tr = useT();
  const { signOut } = useSession();
  const { data: account } = useAccount();
  return async () => {
    const lost = !account?.secured;
    const ok = await askConfirm(tr('security.signOut.confirm'), lost ? tr('security.signOut.lost') : undefined, tr('security.signOut.button'));
    if (ok) await signOut();
  };
}

export function SignOutButton() {
  const t = useTheme();
  const tr = useT();
  const signOut = useSignOut();
  return (
    <Pressable
      accessibilityRole="button"
      onPress={signOut}
      style={{ alignSelf: 'center', flexDirection: 'row', alignItems: 'center', gap: 8, padding: 12 }}>
      <LogOut size={18} color="#FF3B5C" />
      <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: '#FF3B5C' }}>{tr('security.signOut.button')}</Text>
    </Pressable>
  );
}
