import { type ComponentRef, type Ref, type ReactNode, useState, useRef } from 'react';
import { TextInput, type TextInputProps, View, Pressable, Text, Platform } from 'react-native';
import { useTheme } from '@/lib/theme';
import { Card } from '@/components/ui/card';
import { useT } from '@/lib/i18n';
import { EyeOff, Eye } from 'lucide-react-native';

export type InputRef = ComponentRef<typeof TextInput>;

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
  const tr = useT();
  const [shown, setShown] = useState(false);
  return (
    <Field
      placeholder={tr('auth.password')}
      secureTextEntry={!shown}
      autoCapitalize="none"
      autoCorrect={false}
      autoComplete={fresh ? 'new-password' : 'current-password'}
      textContentType={fresh ? 'newPassword' : 'password'}
      {...props}
      right={
        <Pressable
          accessibilityRole="button"
          accessibilityLabel={shown ? tr('auth.field.hide') : tr('auth.field.show')}
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
  const tr = useT();
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
        accessibilityLabel={tr('auth.field.code')}
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
  const tr = useT();
  return (
    <Field
      placeholder="xxxxx-xxxxx"
      accessibilityLabel={tr('auth.field.recovery')}
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
