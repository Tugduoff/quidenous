import { FileUp, Plus, Trash2 } from 'lucide-react-native';
import { Text, TextInput, View } from 'react-native';
import { type Selection, SelectButton, SelectionHeader } from '@/components/ui/selection';
import { TemplateLink } from '@/components/packs/template-link';
import { packFormLabel } from '@/components/packs/pack-tag-picker';
import { useT } from '@/lib/i18n';
import { QUESTION_MAX } from '@/lib/utils/import-draft';
import { useTheme } from '@/lib/theme';
import { PressableScale } from '@/components/ui/pressable-scale';
import { Card } from '@/components/ui/card';

export function PackQuestionTools({
  sel,
  count,
  onClearAll,
  onImportFile,
  packTemplate,
  draft,
  onDraft,
  onAdd,
  draftOk,
}: {
  sel: Selection<string>;
  count: number;
  onClearAll: () => void;
  onImportFile: () => void;
  packTemplate: boolean;
  draft: string;
  onDraft: (v: string) => void;
  onAdd: () => void;
  draftOk: boolean;
}) {
  const t = useTheme();
  const tr = useT();
  const label = packFormLabel(t);
  return (
    <>
      {sel.on ? (
        <SelectionHeader sel={sel} />
      ) : (
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8, marginTop: 4 }}>
          <Text style={[label, { flex: 1 }]}>{tr('packs.questions', { count: count })}</Text>
          {count > 0 && <SelectButton size={32} onPress={() => sel.start()} />}
          {count > 0 && (
            <PressableScale
              accessibilityRole="button"
              accessibilityLabel={tr('packs.clearAll')}
              onPress={onClearAll}
              hitSlop={4}
              style={{ width: 32, height: 32, borderRadius: 16, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
              <Trash2 size={15} color={t.txt} />
            </PressableScale>
          )}
          <PressableScale
            accessibilityRole="button"
            onPress={onImportFile}
            style={{
              flexDirection: 'row',
              alignItems: 'center',
              gap: 6,
              height: 32,
              paddingHorizontal: 12,
              borderRadius: t.radius.pill,
              backgroundColor: t.card,
            }}>
            <FileUp size={15} color={t.txt} />
            <Text style={{ fontFamily: t.font.semibold, fontSize: 13, color: t.txt }}>{tr('packs.fromFile')}</Text>
          </PressableScale>
        </View>
      )}
      {/* What a file's header can fill in, and the format for an AI to write one. */}
      {!sel.on && (
        <View style={{ flexDirection: 'row', alignItems: 'flex-start', gap: 12, marginTop: -8, paddingHorizontal: 4 }}>
          <Text style={{ flex: 1, fontFamily: t.font.regular, fontSize: 12, lineHeight: 17, color: t.mut }}>{tr('packs.fileHeader')}</Text>
          {packTemplate && <TemplateLink style={{ marginTop: 1 }} />}
        </View>
      )}
      {!sel.on && (
        <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 8, padding: 8, paddingLeft: 14, marginTop: -6 }}>
          <TextInput
            accessibilityLabel={tr('packs.addQuestion')}
            placeholder={tr('packs.addQuestion')}
            placeholderTextColor={t.mut}
            value={draft}
            onChangeText={onDraft}
            maxLength={QUESTION_MAX}
            returnKeyType="done"
            submitBehavior="submit"
            onSubmitEditing={onAdd}
            style={{ flex: 1, minWidth: 0, height: 40, fontFamily: t.font.regular, fontSize: 16, color: t.txt }}
          />
          <PressableScale
            accessibilityRole="button"
            accessibilityLabel={tr('common.add')}
            disabled={!draftOk}
            onPress={onAdd}
            hitSlop={4}
            style={{
              width: 40,
              height: 40,
              borderRadius: 20,
              alignItems: 'center',
              justifyContent: 'center',
              backgroundColor: draftOk ? t.acc : t.card2,
            }}>
            <Plus size={20} color={draftOk ? t.accTxt : t.mut} />
          </PressableScale>
        </Card>
      )}
    </>
  );
}
