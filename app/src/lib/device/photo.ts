import { File as FsFile } from 'expo-file-system';
import * as ImageManipulator from 'expo-image-manipulator';
import { Platform } from 'react-native';

import { readUriBlob } from '@/lib/api/client';

const PHOTO_WIDTH = 512;

export async function photoForm(uri: string) {
  const small = await ImageManipulator.manipulateAsync(uri, [{ resize: { width: PHOTO_WIDTH } }], {
    compress: 0.8,
    format: ImageManipulator.SaveFormat.JPEG,
  });
  const form = new FormData();
  if (Platform.OS === 'web') {
    form.append('photo', await readUriBlob(small.uri), 'photo.jpg');
  } else {
    form.append('photo', new FsFile(small.uri), 'photo.jpg');
  }
  return form;
}
