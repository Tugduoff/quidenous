import { router } from 'expo-router';
import { Camera, ChevronRight, Images, type LucideIcon, Smile } from 'lucide-react-native';
import { Pressable, Text } from 'react-native';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { Dialog, DialogTitle } from '@/components/ui/dialog';

function Choice({ icon: Icon, label, onPress }: { icon: LucideIcon; label: string; onPress: () => void }) {
  const t = useTheme();
  return (
    <Pressable
      accessibilityRole="button"
      onPress={onPress}
      style={({ pressed }) => ({
        flexDirection: 'row',
        alignItems: 'center',
        gap: 12,
        height: 52,
        paddingHorizontal: 16,
        borderRadius: 16,
        backgroundColor: t.card2,
        opacity: pressed ? 0.7 : 1,
      })}>
      <Icon size={20} color={t.txt} />
      <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 16, color: t.txt }}>{label}</Text>
      <ChevronRight size={18} color={t.mut} />
    </Pressable>
  );
}

export function PhotoSourceDialog({
  visible,
  onClose,
  avatars,
  onPick,
}: {
  visible: boolean;
  onClose: () => void;
  avatars: boolean;
  onPick: (from: 'library' | 'camera') => void;
}) {
  const tr = useT();
  return (
  <Dialog visible={visible} onClose={onClose}>
    <DialogTitle title={tr('profile.photo.title')} />
    {avatars && (
      <Choice
        icon={Smile}
        label={tr('profile.photo.avatar')}
        onPress={() => {
          onClose();
          router.push('/avatar');
        }}
      />
    )}
    <Choice icon={Images} label={tr('profile.photo.library')} onPress={() => onPick('library')} />
    <Choice icon={Camera} label={tr('profile.photo.camera')} onPress={() => onPick('camera')} />
  </Dialog>
  );
}
