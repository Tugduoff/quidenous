import { Platform, type TextInputProps } from 'react-native';

type KeyPress = Parameters<NonNullable<TextInputProps['onKeyPress']>>[0];

export const SEND_KEY: TextInputProps = { returnKeyType: 'send', submitBehavior: 'submit' };

export function isSendKey(e: KeyPress) {
  const key = e.nativeEvent as typeof e.nativeEvent & { shiftKey?: boolean; isComposing?: boolean };
  return Platform.OS === 'web' && key.key === 'Enter' && !key.shiftKey && !key.isComposing;
}

/**
 * A multiline field whose keyboard key sends: Enter sends, Shift+Enter goes to a new line.
 * The web ignores submitBehavior on a multiline field, so Enter is caught there by hand.
 */
export function sendOnEnter(send: () => void): TextInputProps {
  return {
    ...SEND_KEY,
    onSubmitEditing: send,
    onKeyPress: (e) => {
      if (!isSendKey(e)) return;
      e.preventDefault();
      send();
    },
  };
}
