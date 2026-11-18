import { Platform, Share } from 'react-native';

export async function copyText(text: string): Promise<boolean> {
  try {
    if (Platform.OS === 'web') await navigator.clipboard.writeText(text);
    else await Share.share({ message: text });
    return true;
  } catch {
    return false;
  }
}
