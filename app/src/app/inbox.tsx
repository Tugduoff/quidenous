import { useQueryClient } from '@tanstack/react-query';
import { router } from 'expo-router';
import { BellRing, ChevronLeft, Crown, MessageCircle, Sparkles } from 'lucide-react-native';
import { useEffect, useState } from 'react';
import { Pressable, Text, View } from 'react-native';

import { Screen } from '@/components/screen';
import { Avatar, Card, Display } from '@/components/ui';
import { api, type InboxItem } from '@/lib/api';
import { keys, useInbox, useMembers } from '@/lib/queries';
import { useTheme } from '@/lib/theme';
import { ago } from '@/lib/time';

const ICON = { message: MessageCircle, nudge: BellRing, result: Crown, question: Sparkles } as const;
const TITLE = { message: 'Nouveau message', nudge: 'Relance', result: 'Résultat dispo', question: 'Nouvelle question' } as const;

/** Everything that happened lately: results, new questions, nudges, messages. Opening it marks all as read. */
export default function InboxScreen() {
  const t = useTheme();
  const qc = useQueryClient();
  const { data } = useInbox();
  const { byId } = useMembers();
  const unread = data?.unread ?? 0;
  // What was new when I opened the screen stays highlighted while I look, even once marked as read.
  const [fresh, setFresh] = useState<Set<number> | null>(null);
  useEffect(() => {
    if (data && !fresh) setFresh(new Set(data.items.filter((i) => !i.read).map((i) => i.id)));
  }, [data, fresh]);
  useEffect(() => {
    if (unread > 0) api('/inbox/read', { method: 'POST' }).then(() => qc.invalidateQueries({ queryKey: keys.inbox }));
  }, [unread, qc]);

  const open = (item: InboxItem) => {
    if (item.kind === 'nudge' || item.kind === 'question' || item.round_id == null) router.replace('/');
    else router.push(`/round/${item.round_id}`);
  };

  return (
    <Screen tabs={false}>
      <Pressable
        accessibilityRole="button"
        accessibilityLabel="Retour"
        onPress={() => (router.canGoBack() ? router.back() : router.replace('/'))}
        style={{ width: 40, height: 40, borderRadius: 20, alignItems: 'center', justifyContent: 'center', backgroundColor: t.card }}>
        <ChevronLeft size={22} color={t.txt} />
      </Pressable>
      <Display size={30}>Notifications</Display>
      {data && data.items.length === 0 && (
        <Text style={{ fontFamily: t.font.regular, fontSize: 15, color: t.mut }}>Rien de neuf pour l'instant.</Text>
      )}
      {data?.items.map((item) => {
        const Icon = ICON[item.kind];
        const isNew = !item.read || !!fresh?.has(item.id);
        const actor = item.actor_id != null ? byId.get(item.actor_id) : undefined;
        return (
          <Pressable key={item.id} accessibilityRole="button" onPress={() => open(item)}>
            <Card style={{ flexDirection: 'row', gap: 12, alignItems: 'center', padding: 14 }}>
              {actor ? (
                <Avatar member={actor} size={40} />
              ) : (
                <View style={{ width: 40, height: 40, borderRadius: 20, backgroundColor: t.accSoft, alignItems: 'center', justifyContent: 'center' }}>
                  <Icon size={20} color={t.accInk} />
                </View>
              )}
              <View style={{ flex: 1, gap: 2 }}>
                <Text style={{ fontFamily: t.font.bold, fontSize: 13, color: t.mut }}>
                  {item.kind === 'message' && actor ? actor.name : TITLE[item.kind]} · {ago(item.created_at)}
                </Text>
                <Text numberOfLines={2} style={{ fontFamily: isNew ? t.font.semibold : t.font.regular, fontSize: 15, lineHeight: 20, color: t.txt }}>
                  {item.text}
                </Text>
              </View>
              {isNew && <View style={{ width: 10, height: 10, borderRadius: 5, backgroundColor: t.tag }} />}
            </Card>
          </Pressable>
        );
      })}
    </Screen>
  );
}
