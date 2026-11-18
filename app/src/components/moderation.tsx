import { Ban, Check, Eye, Flag, type LucideIcon, Trash2 } from 'lucide-react-native';
import { type ReactNode, useEffect, useRef, useState } from 'react';
import { Pressable, Text, View } from 'react-native';

import { ErrorText, Field } from '@/components/auth-ui';
import { type Anchor, Dialog, DialogActions, DialogTitle } from '@/components/ui';
import { authError } from '@/lib/auth';
import { useT } from '@/lib/i18n';
import { REASONS, report, useBlockActions, useBlocks, type Blocked, type ReportKind, type ReportReason } from '@/lib/moderation';
import { useTheme } from '@/lib/theme';

const RED = '#FF3B5C';

/**
 * What was long-pressed: a message, a member or a question, and the person behind it when they can be blocked.
 * `onDelete`: I may delete it (my own message, or any message of a room I own). `own`: nothing to report or block.
 * `onRestore`: a message hidden by reports, in a room I own: I may bring it back.
 * `anchor`: the menu floats next to it instead of the middle of the screen.
 */
export type ModTarget = {
  kind: ReportKind;
  id: number;
  person?: Blocked;
  onDelete?: () => Promise<unknown>;
  onRestore?: () => Promise<unknown>;
  own?: boolean;
  anchor?: Anchor;
  /** Extra actions at the top of the menu (a room member: remove, vote to remove). */
  actions?: ModAction[];
};

/** One more line of the menu. `confirm`: asked in the same dialog first. `hint`: quiet text at the end (votes so far). */
export type ModAction = {
  icon: LucideIcon;
  label: string;
  hint?: string;
  danger?: boolean;
  confirm?: { title: string; message?: string; label: string };
  run: () => Promise<unknown>;
};

type Step = 'menu' | 'report' | 'block' | 'delete' | 'action' | 'done';

/**
 * Report (pick a reason) and block / unblock, in one dialog: no second modal on top of the first.
 * `header`: shown at the top of the menu instead of its title (the chat's reactions); the menu then always opens first.
 */
export function ModerationDialog({
  target,
  onClose,
  onReported,
  header,
}: {
  target: ModTarget | null;
  onClose: () => void;
  onReported?: (target: ModTarget) => void;
  header?: (target: ModTarget) => ReactNode;
}) {
  const t = useTheme();
  const tr = useT();
  const { ids } = useBlocks();
  const { block, unblock } = useBlockActions();
  // The last target stays drawn while the dialog fades out.
  const [shown, setShown] = useState<ModTarget | null>(null);
  const [step, setStep] = useState<Step>('menu');
  const [reason, setReason] = useState<ReportReason | null>(null);
  const [details, setDetails] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [action, setAction] = useState<ModAction | null>(null);
  if (target && target !== shown) {
    setShown(target);
    setStep(
      header || target.onRestore || target.actions?.length ? 'menu' : target.own ? 'delete' : target.person || target.onDelete ? 'menu' : 'report',
    );
    setAction(null);
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

  const runAction = (a: ModAction) =>
    run(async () => {
      await a.run();
      onClose();
    });

  const person = shown?.person;
  const blocked = !!person && ids.has(person.user_id);
  const send = () =>
    shown &&
    reason &&
    run(async () => {
      await report(shown.kind, shown.id, reason, reason === 'other' ? details.trim() || undefined : undefined);
      onReported?.(shown);
      setStep('done');
    });

  const what = !shown
    ? ''
    : shown.kind === 'message'
      ? tr('moderation.reportMessage')
      : shown.kind === 'question' || shown.kind === 'suggestion'
        ? tr('moderation.reportQuestion')
        : shown.kind === 'answer'
          ? tr('moderation.reportAnswer')
          : tr('moderation.reportPerson', { name: person?.name ?? '' });

  return (
    <Dialog visible={!!target} onClose={onClose} anchor={step === 'menu' ? shown?.anchor : undefined}>
      {step === 'menu' && (
        <>
          {header && shown ? header(shown) : <DialogTitle title={person?.name ?? what} />}
          {shown?.actions?.map((a) => (
            <Action
              key={a.label}
              icon={a.icon}
              label={a.label}
              hint={a.hint}
              danger={a.danger}
              disabled={busy}
              onPress={() => {
                if (!a.confirm) return runAction(a);
                setAction(a);
                setStep('action');
              }}
            />
          ))}
          {!!shown?.onRestore && (
            <Action
              icon={Eye}
              label={tr('moderation.restore')}
              disabled={busy}
              onPress={() =>
                run(async () => {
                  await shown.onRestore?.();
                  onClose();
                })
              }
            />
          )}
          {!shown?.own && <Action icon={Flag} label={tr('moderation.report')} onPress={() => setStep('report')} />}
          {!!shown?.onDelete && <Action icon={Trash2} label={tr('moderation.delete')} danger onPress={() => setStep('delete')} />}
          {!person ? null : blocked ? (
            <Action
              icon={Ban}
              label={tr('moderation.unblock', { name: person.name })}
              disabled={busy}
              onPress={() =>
                run(async () => {
                  await unblock(person.user_id);
                  onClose();
                })
              }
            />
          ) : (
            <Action icon={Ban} label={tr('moderation.block', { name: person.name })} danger onPress={() => setStep('block')} />
          )}
          <ErrorText>{error}</ErrorText>
          {/* The floating menu closes with a tap beside it. */}
          {!header && (
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
              <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{tr('common.cancel')}</Text>
            </Pressable>
          )}
        </>
      )}
      {step === 'report' && (
        <>
          <DialogTitle title={what} />
          <View accessibilityRole="radiogroup" style={{ gap: 6 }}>
            {REASONS.map((r) => {
              const on = reason === r;
              return (
                <Pressable
                  key={r}
                  accessibilityRole="radio"
                  accessibilityState={{ checked: on }}
                  onPress={() => setReason(r)}
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
                  <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{tr(`moderation.reasons.${r}`)}</Text>
                </Pressable>
              );
            })}
          </View>
          {reason === 'other' && (
            <Field
              inset
              autoFocus
              placeholder={tr('moderation.details')}
              value={details}
              onChangeText={setDetails}
              maxLength={500}
              returnKeyType="send"
              onSubmitEditing={send}
            />
          )}
          <ErrorText>{error}</ErrorText>
          <DialogActions confirmLabel={tr('moderation.report')} onConfirm={send} onCancel={onClose} busy={busy} disabled={!reason} />
        </>
      )}
      {step === 'block' && person && (
        <>
          <DialogTitle title={tr('moderation.blockTitle', { name: person.name })} message={tr('moderation.blockBody')} />
          <ErrorText>{error}</ErrorText>
          <DialogActions
            confirmLabel={tr('moderation.blockConfirm')}
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
      {step === 'delete' && (
        <>
          <DialogTitle title={tr('moderation.deleteTitle')} />
          <ErrorText>{error}</ErrorText>
          <DialogActions
            confirmLabel={tr('moderation.deleteConfirm')}
            busy={busy}
            onCancel={onClose}
            onConfirm={() =>
              run(async () => {
                await shown?.onDelete?.();
                onClose();
              })
            }
          />
        </>
      )}
      {step === 'action' && action?.confirm && (
        <>
          <DialogTitle title={action.confirm.title} message={action.confirm.message} />
          <ErrorText>{error}</ErrorText>
          <DialogActions confirmLabel={action.confirm.label} busy={busy} onCancel={onClose} onConfirm={() => runAction(action)} />
        </>
      )}
      {step === 'done' && (
        <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'center', gap: 10, paddingVertical: 8 }}>
          <Check size={22} color={t.accInk} strokeWidth={3} />
          <Text style={{ fontFamily: t.font.bold, fontSize: 18, color: t.txt }}>{tr('moderation.done')}</Text>
        </View>
      )}
    </Dialog>
  );
}

function Action({
  icon: Icon,
  label,
  hint,
  onPress,
  danger,
  disabled,
}: {
  icon: LucideIcon;
  label: string;
  hint?: string;
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
      {!!hint && <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut }}>{hint}</Text>}
    </Pressable>
  );
}
