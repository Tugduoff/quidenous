import { Platform, type TextInputProps } from 'react-native';

/**
 * A multiline field whose keyboard key sends: Enter sends, Shift+Enter goes to a new line.
 * The web ignores submitBehavior on a multiline field, so Enter is caught there by hand.
 */
export function sendOnEnter(send: () => void): TextInputProps {
  return {
    returnKeyType: 'send',
    submitBehavior: 'submit',
    onSubmitEditing: send,
    onKeyPress: (e) => {
      const key = e.nativeEvent as typeof e.nativeEvent & { shiftKey?: boolean; isComposing?: boolean };
      if (Platform.OS !== 'web' || key.key !== 'Enter' || key.shiftKey || key.isComposing) return;
      e.preventDefault();
      send();
    },
  };
}
