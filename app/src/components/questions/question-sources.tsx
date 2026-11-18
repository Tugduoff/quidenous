import { router } from 'expo-router';
import { FileUp, Lightbulb, Package } from 'lucide-react-native';
import { View } from 'react-native';
import { TemplateLink } from '@/components/packs/template-link';
import { BulkButton } from '@/components/questions/bulk-button';
import { useFeatures } from '@/lib/api/features';
import { useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';

export function QuestionSources({ importing, onPickFile }: { importing: boolean; onPickFile: () => void }) {
  const t = useTheme();
  const tr = useT();
  const { community, packTemplate } = useFeatures();
  return (
    <>
      <View style={{ flexDirection: 'row', gap: 10 }}>
        <BulkButton label={tr('questions.packs')} icon={<Package size={16} color={t.txt} />} onPress={() => router.push('/packs')} />
        <BulkButton
          label={importing ? tr('questions.importing') : tr('questions.import')}
          icon={<FileUp size={16} color={t.txt} />}
          accessibilityLabel={tr('questions.importFile')}
          disabled={importing}
          onPress={onPickFile}
        />
        {community && (
          <BulkButton label={tr('community.entry')} icon={<Lightbulb size={16} color={t.txt} />} onPress={() => router.push('/suggest')} />
        )}
      </View>
      {packTemplate && <TemplateLink style={{ alignSelf: 'center', marginTop: -6 }} />}
    </>
  );
}
