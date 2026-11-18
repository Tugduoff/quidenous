import { Platform, Share } from 'react-native';

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
