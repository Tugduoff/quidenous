import { useState } from 'react';
import { TextInput } from 'react-native';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { Button } from '@/components/ui/button';
import { Dialog, DialogTitle } from '@/components/ui/dialog';

export function NameDialog({ visible, name, onClose, onSave }: { visible: boolean; name: string; onClose: () => void; onSave: (name: string) => void }) {
  const t = useTheme();
  const tr = useT();
  const [draft, setDraft] = useState(name);
  const opening = visible ? name : null;
  const [opened, setOpened] = useState(opening);
  if (opening !== opened) {
    setOpened(opening);
    if (opening != null) setDraft(opening);
  }
  const ok = !!draft.trim() && draft.trim() !== name;
  const submit = () => ok && onSave(draft.trim());
  return (
    <Dialog visible={visible} onClose={onClose}>
      <DialogTitle title={tr('room.settings.nameTitle')} />
      <TextInput
        accessibilityLabel={tr('room.settings.nameTitle')}
        value={draft}
        onChangeText={setDraft}
        maxLength={40}
        autoFocus
        returnKeyType="done"
        onSubmitEditing={submit}
        style={{
          height: 48,
          paddingHorizontal: 14,
          borderRadius: 14,
          backgroundColor: t.card2,
          fontFamily: t.font.semibold,
          fontSize: 16,
          color: t.txt,
        }}
      />
      <Button disabled={!ok} onPress={submit} style={{ height: 48 }}>
        {tr('room.settings.rename')}
      </Button>
    </Dialog>
  );
}
