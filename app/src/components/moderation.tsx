import { Ban, Check, Flag, type LucideIcon } from 'lucide-react-native';
import { useEffect, useRef, useState } from 'react';
import { Pressable, Text, View } from 'react-native';

import { ErrorText, Field } from '@/components/auth-ui';
import { Dialog, DialogActions, DialogTitle } from '@/components/ui';
import { authError } from '@/lib/auth';
import { REASONS, report, useBlockActions, useBlocks, type Blocked, type ReportKind, type ReportReason } from '@/lib/moderation';
import { useTheme } from '@/lib/theme';

const RED = '#FF3B5C';

/** What was long-pressed: a message, a member or a question, and the person behind it when they can be blocked. */
export type ModTarget = { kind: ReportKind; id: number; person?: Blocked };

type Step = 'menu' | 'report' | 'block' | 'done';

/** Report (pick a reason) and block / unblock, in one dialog: no second modal on top of the first. */
export function ModerationDialog({ target, onClose }: { target: ModTarget | null; onClose: () => void }) {
  const t = useTheme();
  const { ids } = useBlocks();
  const { block, unblock } = useBlockActions();
  // The last target stays drawn while the dialog fades out.
  const [shown, setShown] = useState<ModTarget | null>(null);
  const [step, setStep] = useState<Step>('menu');
  const [reason, setReason] = useState<ReportReason | null>(null);
  const [details, setDetails] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  if (target && target !== shown) {
    setShown(target);
    setStep(target.person ? 'menu' : 'report');
    setReason(null);
    setDetails('');
    setError(null);
  }

  // "Signalé" for a moment, then the dialog closes by itself.
  const close = useRef(onClose);
  useEffect(() => {
    close.current = onClose;
  });
  useEffect(() => {
    if (step !== 'done') return;
    const id = setTimeout(() => close.current(), 1200);
    return () => clearTimeout(id);
  }, [step]);

  const run = async (f: () => Promise<void>) => {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      await f();
    } catch (e) {
      setError(authError(e));
    } finally {
      setBusy(false);
    }
  };

  const person = shown?.person;
  const blocked = !!person && ids.has(person.user_id);
  const send = () =>
    shown &&
    reason &&
    run(async () => {
      await report(shown.kind, shown.id, reason, reason === 'other' ? details.trim() || undefined : undefined);
      setStep('done');
    });

  const what = !shown
    ? ''
    : shown.kind === 'message'
      ? 'Signaler ce message'
      : shown.kind === 'question'
        ? 'Signaler cette question'
        : `Signaler ${person?.name ?? ''}`;

  return (
    <Dialog visible={!!target} onClose={onClose}>
      {step === 'menu' && person && (
        <>
          <DialogTitle title={person.name} />
          <Action icon={Flag} label="Signaler" onPress={() => setStep('report')} />
          {blocked ? (
            <Action
              icon={Ban}
              label={`Débloquer ${person.name}`}
              disabled={busy}
              onPress={() =>
                run(async () => {
                  await unblock(person.user_id);
                  onClose();
                })
              }
            />
          ) : (
            <Action icon={Ban} label={`Bloquer ${person.name}`} danger onPress={() => setStep('block')} />
          )}
          <ErrorText>{error}</ErrorText>
          <Pressable
            accessibilityRole="button"
            onPress={onClose}
            style={{
              height: 46,
              marginTop: 6,
              borderRadius: t.radius.pill,
              alignItems: 'center',
              justifyContent: 'center',
              backgroundColor: t.card2,
            }}>
            <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>Annuler</Text>
          </Pressable>
        </>
      )}
      {step === 'report' && (
        <>
          <DialogTitle title={what} />
          <View accessibilityRole="radiogroup" style={{ gap: 6 }}>
            {REASONS.map((r) => {
              const on = reason === r.value;
              return (
                <Pressable
                  key={r.value}
                  accessibilityRole="radio"
                  accessibilityState={{ checked: on }}
                  onPress={() => setReason(r.value)}
                  style={{
                    flexDirection: 'row',
                    alignItems: 'center',
                    gap: 12,
                    height: 44,
                    paddingHorizontal: 14,
                    borderRadius: 14,
                    backgroundColor: t.card2,
                  }}>
                  <View
                    style={{
                      width: 20,
                      height: 20,
                      borderRadius: 10,
                      alignItems: 'center',
                      justifyContent: 'center',
                      backgroundColor: on ? t.acc : 'transparent',
                      borderWidth: on ? 0 : 2,
                      borderColor: t.line,
                    }}>
                    {on && <Check size={13} color={t.accTxt} strokeWidth={3} />}
                  </View>
                  <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{r.label}</Text>
                </Pressable>
              );
            })}
          </View>
          {reason === 'other' && (
            <Field
              inset
              autoFocus
              placeholder="Précise (facultatif)"
              value={details}
              onChangeText={setDetails}
              maxLength={500}
              returnKeyType="send"
              onSubmitEditing={send}
            />
          )}
          <ErrorText>{error}</ErrorText>
          <DialogActions confirmLabel="Signaler" onConfirm={send} onCancel={onClose} busy={busy} disabled={!reason} />
        </>
      )}
      {step === 'block' && person && (
        <>
          <DialogTitle title={`Bloquer ${person.name} ?`} message="Tu ne verras plus ses messages." />
          <ErrorText>{error}</ErrorText>
          <DialogActions
            confirmLabel="Bloquer"
            busy={busy}
            onCancel={onClose}
            onConfirm={() =>
              run(async () => {
                await block(person);
                onClose();
              })
            }
          />
        </>
      )}
      {step === 'done' && (
        <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'center', gap: 10, paddingVertical: 8 }}>
          <Check size={22} color={t.accInk} strokeWidth={3} />
          <Text style={{ fontFamily: t.font.bold, fontSize: 18, color: t.txt }}>Signalé</Text>
        </View>
      )}
    </Dialog>
  );
}

function Action({
  icon: Icon,
  label,
  onPress,
  danger,
  disabled,
}: {
  icon: LucideIcon;
  label: string;
  onPress: () => void;
  danger?: boolean;
  disabled?: boolean;
}) {
  const t = useTheme();
  const color = danger ? RED : t.txt;
  return (
    <Pressable
      accessibilityRole="button"
      disabled={disabled}
      onPress={onPress}
      style={({ pressed }) => ({
        flexDirection: 'row',
        alignItems: 'center',
        gap: 12,
        height: 48,
        paddingHorizontal: 14,
        borderRadius: 14,
        backgroundColor: t.card2,
        opacity: pressed || disabled ? 0.7 : 1,
      })}>
      <Icon size={18} color={color} />
      <Text numberOfLines={1} style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color }}>
        {label}
      </Text>
    </Pressable>
  );
}
