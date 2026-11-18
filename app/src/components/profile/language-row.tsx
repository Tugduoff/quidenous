import { useQueryClient } from '@tanstack/react-query';
import { Check, ChevronRight, Languages } from 'lucide-react-native';
import { useState } from 'react';
import { Pressable, Text } from 'react-native';
import { type LangPref, setLangPref, type TKey, useLang, useT } from '@/lib/i18n';
import { useTheme } from '@/lib/theme';
import { Card } from '@/components/ui/card';
import { Dialog, DialogTitle } from '@/components/ui/dialog';

/** Profile > Langue: follow the phone, or a fixed language (each named in its own language). */
const LANGS: { pref: LangPref; label: TKey }[] = [
  { pref: 'auto', label: 'profile.language.auto' },
  { pref: 'fr', label: 'profile.language.fr' },
  { pref: 'en', label: 'profile.language.en' },
];

/** One of the language choices: a row, ticked when it's the current one. */
function LangOption({ label, on, onPress }: { label: string; on: boolean; onPress: () => void }) {
  const t = useTheme();
  return (
    <Pressable
      accessibilityRole="radio"
      accessibilityState={{ selected: on }}
      onPress={onPress}
      style={({ pressed }) => ({
        flexDirection: 'row',
        alignItems: 'center',
        gap: 12,
        height: 52,
        paddingHorizontal: 16,
        borderRadius: 16,
        borderWidth: 2,
        borderColor: on ? t.accInk : 'transparent',
        backgroundColor: on ? t.accSoft : t.card2,
        opacity: pressed ? 0.7 : 1,
      })}>
      <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 16, color: on ? t.accInk : t.txt }}>{label}</Text>
      {on && <Check size={18} color={t.accInk} />}
    </Pressable>
  );
}

export function LanguageRow() {
  const t = useTheme();
  const tr = useT();
  const qc = useQueryClient();
  const { pref: langPref } = useLang();
  const [langOpen, setLangOpen] = useState(false);
  return (
    <>
    <Pressable accessibilityRole="button" onPress={() => setLangOpen(true)}>
      <Card style={{ flexDirection: 'row', alignItems: 'center', gap: 12, padding: 16 }}>
        <Languages size={20} color={t.txt} />
        <Text style={{ flex: 1, fontFamily: t.font.semibold, fontSize: 15, color: t.txt }}>{tr('profile.language.title')}</Text>
        <Text style={{ fontFamily: t.font.semibold, fontSize: 14, color: t.mut }}>{tr((LANGS.find((l) => l.pref === langPref) ?? LANGS[0]).label)}</Text>
        <ChevronRight size={20} color={t.mut} />
      </Card>
    </Pressable>
    <Dialog visible={langOpen} onClose={() => setLangOpen(false)}>
      <DialogTitle title={tr('profile.language.title')} />
      {LANGS.map(({ pref: p, label }) => (
        <LangOption
          key={p}
          label={tr(label)}
          on={p === langPref}
          onPress={() => {
            setLangOpen(false);
            // Server texts (stats stories, inbox) follow the account's language: refetch once it knows.
            if (p !== langPref)
              setLangPref(p).then(
                () => qc.invalidateQueries(),
                () => {},
              );
          }}
        />
      ))}
    </Dialog>
    </>
  );
}
