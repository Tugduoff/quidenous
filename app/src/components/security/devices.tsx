import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Globe, Lock, LogOut, Smartphone, X } from 'lucide-react-native';
import { useState } from 'react';
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
import { lockSupported, setLock, useLock } from '@/lib/lock';
import { useSession } from '@/lib/session';
import { oauth } from '@/lib/sign-in';
import { useTheme } from '@/lib/theme';
import { ago } from '@/lib/time';

/** Google / Discord: link (the web leaves for the provider and comes back on /oauth), unlink. */
export function ProvidersSection({ account }: { account: Account }) {
  const t = useTheme();
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
    if (await askConfirm(`Délier ${PROVIDER_NAMES[p]} ?`, undefined, 'Délier')) run(p, () => auth.unlink(p));
  };

  return (
    <Section title="Comptes liés">
      {providers.map((p) => {
        const linked = account.providers.includes(p);
        return (
          <Row
            key={p}
            icon={<BrandIcon provider={p} />}
            title={PROVIDER_NAMES[p]}
            sub={linked ? 'Lié' : null}
            right={
              linked ? (
                <Pill tone="danger" onPress={() => unlink(p)} loading={busy === p} disabled={!!busy}>
                  Délier
                </Pill>
              ) : (
                <Pill onPress={() => link(p)} loading={busy === p} disabled={!!busy}>
                  Lier
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
  if (!s.device) return 'Appareil inconnu';
  try {
    return decodeURIComponent(s.device);
  } catch {
    return s.device;
  }
}

/** Where I'm signed in: this device first, sign one out, or every other one. */
export function SessionsSection() {
  const t = useTheme();
  const qc = useQueryClient();
  const { data: sessions } = useQuery({ queryKey: ['sessions'], queryFn: auth.sessions });
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const refresh = () => qc.invalidateQueries({ queryKey: ['sessions'] });
  const list = [...(sessions ?? [])].sort((a, b) => Number(b.current) - Number(a.current));
  const others = list.filter((s) => !s.current).length;

  const revoke = async (s: DeviceSession) => {
    if (!(await askConfirm('Déconnecter cet appareil ?', label(s), 'Déconnecter'))) return;
    setError(null);
    await auth.revoke(s.id).catch((e) => setError(authError(e)));
    refresh();
  };
  const revokeOthers = async () => {
    if (!(await askConfirm('Déconnecter les autres appareils ?', undefined, 'Déconnecter'))) return;
    setBusy(true);
    setError(null);
    await auth.revokeOthers().catch((e) => setError(authError(e)));
    setBusy(false);
    refresh();
  };

  if (!sessions) return null;
  return (
    <Section title="Appareils">
      {list.map((s) => {
        const Icon = label(s).startsWith('Web') ? Globe : Smartphone;
        return (
          <Row
            key={s.id}
            icon={<Icon size={20} color={s.current ? t.accInk : t.mut} />}
            title={label(s)}
            sub={s.current ? 'Cet appareil' : `Actif ${ago(s.last_used_at ?? s.created_at)}`}
            right={
              !s.current && (
                <Pressable
                  accessibilityRole="button"
                  accessibilityLabel={`Déconnecter ${label(s)}`}
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
          Déconnecter les autres appareils
        </TextLink>
      )}
    </Section>
  );
}

/** Face ID / fingerprint when coming back to the app (phones with the native module only). */
export function LockSection() {
  const t = useTheme();
  const { enabled } = useLock();
  const [error, setError] = useState<string | null>(null);
  if (!lockSupported) return null;
  const toggle = async (on: boolean) => {
    setError(null);
    const r = await setLock(on).catch(() => 'failed' as const);
    if (r === 'unavailable') setError("Active d'abord un code ou Face ID sur ton téléphone");
  };
  return (
    <Section title="Verrouillage">
      <Row
        icon={<Lock size={20} color={enabled ? t.accInk : t.mut} />}
        title="Verrouiller l'app"
        right={<Toggle label="" value={enabled} onChange={toggle} />}
      />
      <ErrorText>{error}</ErrorText>
    </Section>
  );
}

/** Sign out, after a warning when the account has no way back in (legacy, token only). */
export function useSignOut() {
  const { signOut } = useSession();
  const { data: account } = useAccount();
  return async () => {
    const lost = !account?.secured;
    const ok = await askConfirm(
      'Se déconnecter ?',
      lost ? 'Sans e-mail ni compte lié, tu ne pourras plus revenir sur ce compte.' : undefined,
      'Se déconnecter',
    );
    if (ok) await signOut();
  };
}

export function SignOutButton() {
  const t = useTheme();
  const signOut = useSignOut();
  return (
    <Pressable
      accessibilityRole="button"
      onPress={signOut}
      style={{ alignSelf: 'center', flexDirection: 'row', alignItems: 'center', gap: 8, padding: 12 }}>
      <LogOut size={18} color="#FF3B5C" />
      <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: '#FF3B5C' }}>Se déconnecter</Text>
    </Pressable>
  );
}
