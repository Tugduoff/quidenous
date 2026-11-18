import { useQueryClient } from '@tanstack/react-query';
import * as ImagePicker from 'expo-image-picker';
import { useState } from 'react';
import { Platform } from 'react-native';
import { ApiError } from '@/lib/api/client';
import { useT } from '@/lib/i18n';
import { useSession } from '@/lib/session';
import { type Member, groupKeys } from '@/lib/api/group';
import { uploadMyPhoto, setMyName } from '@/lib/api/me';
import { photoForm } from '@/lib/device/photo';

/** Profile: my photo (library or camera) and my name on my seat, with what is busy and the last error. */
export function useProfileEdit() {
  const tr = useT();
  const qc = useQueryClient();
  const { me, setMe } = useSession();
  const [name, setName] = useState(me?.name ?? '');
  const [busy, setBusy] = useState<'photo' | 'name' | null>(null);
  const [error, setError] = useState<string | null>(null);

  const updated = (m: Member) => {
    setMe(m);
    qc.invalidateQueries({ queryKey: groupKeys.all });
  };

  const pickPhoto = async (from: 'library' | 'camera') => {
    setError(null);
    try {
      const options: ImagePicker.ImagePickerOptions = { mediaTypes: ['images'], allowsEditing: true, aspect: [1, 1], quality: 1 };
      if (from === 'camera' && Platform.OS !== 'web' && !(await ImagePicker.requestCameraPermissionsAsync()).granted) {
        setError(tr('profile.photo.cameraDenied'));
        return;
      }
      const picked = await (from === 'camera' ? ImagePicker.launchCameraAsync(options) : ImagePicker.launchImageLibraryAsync(options));
      if (picked.canceled || !picked.assets[0]) return;
      setBusy('photo');
      updated(await uploadMyPhoto(await photoForm(picked.assets[0].uri)));
    } catch (e) {
      // Say why, so a failure on someone's phone can be told apart (server refusal vs. on the phone).
      setError(
        tr('profile.photo.failed', {
          reason: e instanceof ApiError ? e.status : tr('profile.photo.error'),
        }),
      );
    } finally {
      setBusy(null);
    }
  };

  const saveName = async () => {
    setBusy('name');
    setError(null);
    try {
      updated(await setMyName(name));
    } catch {
      setError(tr('profile.badName'));
    } finally {
      setBusy(null);
    }
  };

  return { name, setName, busy, error, pickPhoto, saveName };
}
