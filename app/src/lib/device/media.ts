import type { ImagePickerAsset } from 'expo-image-picker';
import { ImageManipulator, SaveFormat } from 'expo-image-manipulator';
import { Platform } from 'react-native';
import type { Message } from '@/lib/api/chat';
import { mediaUrl, readUriBlob } from '@/lib/api/client';
import { Directory, Paths, File as FsFile } from 'expo-file-system';
import * as Sharing from 'expo-sharing';
import { mediaOf } from '@/lib/utils/chat';

/** Longest side of a photo sent in the chat (the server resizes again, to the same size). */
const MAX_SIDE = 1600;

/** Decoded at most this big before the resize: a 48 MP photo never sits whole in memory. */
const DECODE_MAX = 3200;

/** The server's cap on one upload. */
const MAX_BYTES = 8 * 1024 * 1024;

/** A photo ready to send: a file on the phone (a blob: or data: URL on the web), its size, its type. */
export type ImageDraft = { uri: string; width: number; height: number; mime: string };

/**
 * A picked photo as it leaves the phone: at most 1600 px on its longest side, re-encoded as JPEG 0.8, which leaves its
 * EXIF (GPS, camera) behind. A GIF goes as it is (animated; GIF has no EXIF), within the server's 8 MB.
 */
export async function prepareImage(asset: ImagePickerAsset): Promise<ImageDraft | 'too_large'> {
  if (asset.mimeType === 'image/gif') {
    if (asset.fileSize && asset.fileSize > MAX_BYTES) return 'too_large';
    return { uri: asset.uri, width: asset.width || 1, height: asset.height || 1, mime: 'image/gif' };
  }
  let ref = await ImageManipulator.manipulate(asset.uri, { maxWidth: DECODE_MAX, maxHeight: DECODE_MAX }).renderAsync();
  if (Math.max(ref.width, ref.height) > MAX_SIDE) {
    const size = ref.width >= ref.height ? { width: MAX_SIDE } : { height: MAX_SIDE };
    ref = await ImageManipulator.manipulate(ref).resize(size).renderAsync();
  }
  const out = await ref.saveAsync({ compress: 0.8, format: SaveFormat.JPEG });
  return { uri: out.uri, width: out.width, height: out.height, mime: 'image/jpeg' };
}

const EXT: Record<string, string> = { 'image/jpeg': 'jpg', 'image/png': 'png', 'image/webp': 'webp', 'image/gif': 'gif' };

/** The web can hand a file to the system's share sheet (otherwise the viewer's button downloads it). */
export const canShareFiles = () => Platform.OS !== 'web' || (typeof navigator !== 'undefined' && typeof navigator.canShare === 'function');

/**
 * Share or keep a chat photo / GIF. Phones: downloaded to the cache, then the share sheet (which offers to save it).
 * Web: the share sheet with the file when the browser has one, a download otherwise.
 */
export async function shareMedia(m: Message): Promise<void> {
  const media = mediaOf(m);
  if (!media) return;
  const url = mediaUrl(media.url);
  if (Platform.OS === 'web') {
    const blob = await readUriBlob(url);
    const mime = blob.type || media.mime || (m.kind === 'gif' ? 'image/gif' : 'image/jpeg');
    const name = `qui-de-nous-${m.id}.${EXT[mime] ?? 'jpg'}`;
    if (typeof File !== 'undefined' && canShareFiles()) {
      const file = new File([blob], name, { type: mime });
      if (navigator.canShare({ files: [file] })) {
        try {
          await navigator.share({ files: [file] });
          return;
        } catch (e) {
          // Closing the share sheet is an answer, not a failure.
          if (e instanceof Error && e.name === 'AbortError') return;
        }
      }
    }
    const href = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = href;
    a.download = name;
    a.click();
    setTimeout(() => URL.revokeObjectURL(href), 30_000);
    return;
  }
  const mime = media.mime || (m.kind === 'gif' ? 'image/gif' : 'image/jpeg');
  const dir = new Directory(Paths.cache, 'chat');
  if (!dir.exists) dir.create({ intermediates: true });
  const file = new FsFile(dir, `qui-de-nous-${m.id}.${EXT[mime] ?? 'jpg'}`);
  if (!file.exists) await FsFile.downloadFileAsync(url, file, { idempotent: true });
  await Sharing.shareAsync(file.uri, { mimeType: mime });
}
