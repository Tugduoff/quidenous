import { router } from 'expo-router';
import { ChevronLeft, Eye, EyeOff } from 'lucide-react-native';
import { type ComponentRef, type ReactNode, type Ref, useEffect, useRef, useState } from 'react';
import { ActivityIndicator, Platform, Pressable, Share, Text, TextInput, type TextInputProps, View, type ViewStyle } from 'react-native';

import { Screen } from '@/components/screen';
import { Card, Display } from '@/components/ui';
import { useTheme } from '@/lib/theme';

export type InputRef = ComponentRef<typeof TextInput>;

export function BackButton({ onPress }: { onPress?: () => void }) {
  const t = useTheme();
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityLabel="Retour"
      onPress={onPress ?? (() => (router.canGoBack() ? router.back() : router.replace('/')))}
      style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
      <ChevronLeft size={22} color={t.txt} />
    </Pressable>
  );
}

/** A sign-in step: back button, big title, an optional grey line, the fields, the action at the bottom. */
export function AuthScreen({
  title,
  sub,
  children,
  footer,
  back = true,
}: {
  title: string;
  sub?: ReactNode;
  children?: ReactNode;
  footer?: ReactNode;
  back?: boolean;
}) {
  const t = useTheme();
  return (
    <Screen tabs={false} footer={footer}>
      {back && <BackButton />}
      <View style={{ gap: 8, marginTop: back ? 4 : 48, marginBottom: 4 }}>
        <Display size={32}>{title}</Display>
        {!!sub && <Text style={{ fontFamily: t.font.regular, fontSize: 15, lineHeight: 21, color: t.mut }}>{sub}</Text>}
      </View>
      {children}
    </Screen>
  );
}

/** A text field in its own card; `inset` = inside a card already (second-level background). */
export function Field({ ref, style, right, inset, ...props }: TextInputProps & { ref?: Ref<InputRef>; right?: ReactNode; inset?: boolean }) {
  const t = useTheme();
  const Box = inset ? View : Card;
  return (
    <Box style={{ flexDirection: 'row', alignItems: 'center', ...(inset && { backgroundColor: t.card2, borderRadius: 14 }) }}>
      <TextInput
        ref={ref}
        placeholderTextColor={t.mut}
        submitBehavior="submit"
        accessibilityLabel={props.accessibilityLabel ?? props.placeholder}
        {...props}
        style={[{ flex: 1, minWidth: 0, fontFamily: t.font.semibold, fontSize: 16, color: t.txt, padding: 16 }, style]}
      />
      {right}
    </Box>
  );
}

/** Password with a show/hide eye. `fresh` = a new password (the phone may offer a strong one). */
export function PasswordField({ fresh, ...props }: TextInputProps & { ref?: Ref<InputRef>; fresh?: boolean; inset?: boolean }) {
  const t = useTheme();
  const [shown, setShown] = useState(false);
  return (
    <Field
      placeholder="Mot de passe"
      secureTextEntry={!shown}
      autoCapitalize="none"
      autoCorrect={false}
      autoComplete={fresh ? 'new-password' : 'current-password'}
      textContentType={fresh ? 'newPassword' : 'password'}
      {...props}
      right={
        <Pressable
          accessibilityRole="button"
          accessibilityLabel={shown ? 'Masquer le mot de passe' : 'Afficher le mot de passe'}
          onPress={() => setShown((s) => !s)}
          hitSlop={8}
          style={{ paddingHorizontal: 16, alignSelf: 'stretch', justifyContent: 'center' }}>
          {shown ? <EyeOff size={20} color={t.mut} /> : <Eye size={20} color={t.mut} />}
        </Pressable>
      }
    />
  );
}

export const CODE_LENGTH = 6;

/** Six boxes for a 6-digit code; one hidden field underneath takes the typing, pasting and the phone's autofill. */
export function CodeField({
  value,
  onChange,
  onDone,
  autoFocus = true,
  inset,
}: {
  value: string;
  onChange: (v: string) => void;
  onDone?: (v: string) => void;
  autoFocus?: boolean;
  inset?: boolean;
}) {
  const t = useTheme();
  const input = useRef<InputRef>(null);
  const [focused, setFocused] = useState(autoFocus);
  return (
    <View style={{ flexDirection: 'row', gap: 8, justifyContent: 'center' }}>
      {Array.from({ length: CODE_LENGTH }, (_, i) => {
        const active = focused && i === Math.min(value.length, CODE_LENGTH - 1);
        return (
          <View
            key={i}
            style={[
              { flex: 1, maxWidth: 54, height: 62, borderRadius: 16, alignItems: 'center', justifyContent: 'center', borderWidth: 2 },
              { backgroundColor: inset ? t.card2 : t.card, borderColor: active ? t.accInk : 'transparent' },
              !inset && t.shadow,
            ]}>
            <Text style={{ fontFamily: t.font.bold, fontSize: 26, color: t.txt }}>{value[i] ?? ''}</Text>
          </View>
        );
      })}
      <TextInput
        ref={input}
        accessibilityLabel="Code à 6 chiffres"
        value={value}
        onChangeText={(v) => {
          const digits = v.replace(/\D/g, '').slice(0, CODE_LENGTH);
          onChange(digits);
          if (digits.length === CODE_LENGTH && digits !== value) onDone?.(digits);
        }}
        onSubmitEditing={() => value.length === CODE_LENGTH && onDone?.(value)}
        onFocus={() => setFocused(true)}
        onBlur={() => setFocused(false)}
        autoFocus={autoFocus}
        keyboardType="number-pad"
        inputMode="numeric"
        textContentType="oneTimeCode"
        autoComplete="one-time-code"
        returnKeyType="done"
        caretHidden
        contextMenuHidden={Platform.OS !== 'web'}
        style={{ position: 'absolute', top: 0, left: 0, right: 0, bottom: 0, opacity: 0, fontSize: 16, color: 'transparent' }}
      />
    </View>
  );
}

/** Recovery code field (`xxxxx-xxxxx`), spaced out like the code boxes. */
export function RecoveryField({ value, onChange, onDone }: { value: string; onChange: (v: string) => void; onDone: () => void }) {
  return (
    <Field
      placeholder="xxxxx-xxxxx"
      accessibilityLabel="Code de secours"
      value={value}
      onChangeText={(v) => onChange(v.toLowerCase().replace(/\s/g, ''))}
      autoCapitalize="none"
      autoCorrect={false}
      autoFocus
      maxLength={16}
      returnKeyType="go"
      onSubmitEditing={onDone}
      style={{ textAlign: 'center', letterSpacing: 4, fontSize: 20 }}
    />
  );
}

/** A secondary pill button (card colour), with an icon. */
export function SoftButton({
  children,
  icon,
  onPress,
  loading,
  disabled,
  style,
}: {
  children: string;
  icon?: ReactNode;
  onPress: () => void;
  loading?: boolean;
  disabled?: boolean;
  style?: ViewStyle;
}) {
  const t = useTheme();
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={children}
      disabled={disabled || loading}
      onPress={onPress}
      style={({ pressed }) => [
        { height: 58, borderRadius: 999, flexDirection: 'row', alignItems: 'center', justifyContent: 'center', gap: 10 },
        { backgroundColor: t.card, borderWidth: 1, borderColor: t.line, opacity: pressed || disabled ? 0.7 : 1 },
        style,
      ]}>
      {loading ? <ActivityIndicator color={t.mut} /> : icon}
      <Text style={{ fontFamily: t.font.semibold, fontSize: 16, color: t.txt }}>{children}</Text>
    </Pressable>
  );
}

/** A text link (accent colour). */
export function TextLink({ children, onPress, disabled, style }: { children: string; onPress: () => void; disabled?: boolean; style?: ViewStyle }) {
  const t = useTheme();
  return (
    <Pressable accessibilityRole="button" onPress={onPress} disabled={disabled} hitSlop={8} style={[{ alignSelf: 'center', padding: 8 }, style]}>
      <Text style={{ fontFamily: t.font.semibold, fontSize: 15, color: disabled ? t.mut : t.accInk }}>{children}</Text>
    </Pressable>
  );
}

export function ErrorText({ children }: { children: string | null }) {
  const t = useTheme();
  if (!children) return null;
  return <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.tag, marginLeft: 4 }}>{children}</Text>;
}

/** "Renvoyer" waits a bit between two emails: seconds left, and a way to start the wait. */
export function useCooldown(seconds = 30): [number, () => void] {
  const [left, setLeft] = useState(0);
  useEffect(() => {
    if (!left) return;
    const id = setTimeout(() => setLeft((l) => l - 1), 1000);
    return () => clearTimeout(id);
  }, [left]);
  return [left, () => setLeft(seconds)];
}

/** A titled group of rows (Sécurité screen). */
export function Section({ title, children }: { title: string; children: ReactNode }) {
  const t = useTheme();
  return (
    <View style={{ gap: 8 }}>
      <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.mut, marginLeft: 4 }}>{title}</Text>
      <Card style={{ padding: 16, gap: 14 }}>{children}</Card>
    </View>
  );
}

/** Icon, a title with an optional grey line, and something on the right (a Pill, a check). */
export function Row({ icon, title, sub, right }: { icon?: ReactNode; title: string; sub?: string | null; right?: ReactNode }) {
  const t = useTheme();
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 12, minHeight: 36 }}>
      {icon}
      <View style={{ flex: 1, minWidth: 0, gap: 2 }}>
        <Text numberOfLines={1} style={{ fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>
          {title}
        </Text>
        {!!sub && (
          <Text numberOfLines={1} style={{ fontFamily: t.font.regular, fontSize: 13, color: t.mut }}>
            {sub}
          </Text>
        )}
      </View>
      {right}
    </View>
  );
}

/** A small pill action; `tone` acc = main action, danger = red text. */
export function Pill({
  children,
  onPress,
  loading,
  disabled,
  tone = 'soft',
}: {
  children: string;
  onPress: () => void;
  loading?: boolean;
  disabled?: boolean;
  tone?: 'soft' | 'acc' | 'danger';
}) {
  const t = useTheme();
  const acc = tone === 'acc' && !disabled;
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={children}
      onPress={onPress}
      disabled={disabled || loading}
      hitSlop={4}
      style={({ pressed }) => ({
        height: 36,
        paddingHorizontal: 14,
        borderRadius: 999,
        alignItems: 'center',
        justifyContent: 'center',
        backgroundColor: acc ? t.acc : t.card2,
        opacity: pressed ? 0.8 : 1,
      })}>
      {loading ? (
        <ActivityIndicator size="small" color={acc ? t.accTxt : t.mut} />
      ) : (
        <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: acc ? t.accTxt : tone === 'danger' ? '#FF3B5C' : disabled ? t.mut : t.txt }}>
          {children}
        </Text>
      )}
    </Pressable>
  );
}

/** Copy on the web; the share sheet on a phone (it has "Copy"), so no new native module is needed. */
export async function copyText(text: string): Promise<boolean> {
  try {
    if (Platform.OS === 'web') await navigator.clipboard.writeText(text);
    else await Share.share({ message: text });
    return true;
  } catch {
    return false;
  }
}
