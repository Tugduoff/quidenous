import type { ReactNode } from 'react';
import { Text, View } from 'react-native';
import { Screen } from '@/components/ui/screen';
import { useTheme } from '@/lib/theme';
import { Display } from '@/components/ui/text';
import { BackButton } from '@/components/ui/back-button';

/** A sign-in step: back button, big title, an optional grey line, the fields, the action at the bottom. */
export function AuthScreen({
  title,
  sub,
  children,
  footer,
  back = true,
  oneLine,
}: {
  title: string;
  sub?: ReactNode;
  /** The grey line holds an e-mail: one line, cut with an ellipsis. */
  oneLine?: boolean;
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
        {!!sub && (
          <Text numberOfLines={oneLine ? 1 : undefined} style={{ fontFamily: t.font.regular, fontSize: 15, lineHeight: 21, color: t.mut }}>
            {sub}
          </Text>
        )}
      </View>
      {children}
    </Screen>
  );
}
