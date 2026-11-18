import { TextInput } from 'react-native';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import type { Member } from '@/lib/api/group';

export function NameCard({ me, name, onName, saving, onSave }: { me: Member; name: string; onName: (v: string) => void; saving: boolean; onSave: () => void }) {
  const t = useTheme();
  const tr = useT();
  return (
    <Card style={{ flexDirection: 'row', alignItems: 'center', paddingRight: 8 }}>
      <TextInput
        accessibilityLabel={tr('profile.name')}
        value={name}
        onChangeText={onName}
        maxLength={24}
        returnKeyType="done"
        onSubmitEditing={() => name.trim() && name.trim() !== me?.name && onSave()}
        style={{ flex: 1, minWidth: 0, padding: 16, fontFamily: t.font.semibold, fontSize: 16, color: t.txt }}
      />
      <Button
        onPress={onSave}
        loading={saving}
        disabled={!name.trim() || name.trim() === me?.name}
        style={{ height: 44, paddingHorizontal: 18 }}>
        {tr('profile.saveName')}
      </Button>
    </Card>
  );
}
