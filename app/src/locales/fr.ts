// French: the source of truth. Every key here must exist in en.ts (tsc checks it). See docs/I18N.md.
import { common } from '@/locales/fr/common';
import { vote } from '@/locales/fr/vote';
import { results } from '@/locales/fr/results';
import { chat } from '@/locales/fr/chat';
import { moderation } from '@/locales/fr/moderation';
import { celebration } from '@/locales/fr/celebration';
import { questions } from '@/locales/fr/questions';
import { history } from '@/locales/fr/history';
import { stats } from '@/locales/fr/stats';
import { packs } from '@/locales/fr/packs';
import { share } from '@/locales/fr/share';
import { auth } from '@/locales/fr/auth';
import { security } from '@/locales/fr/security';
import { errors } from '@/locales/fr/errors';
import { install } from '@/locales/fr/install';
import { update } from '@/locales/fr/update';
import { lock } from '@/locales/fr/lock';
import { profile } from '@/locales/fr/profile';
import { avatar } from '@/locales/fr/avatar';
import { friends } from '@/locales/fr/friends';
import { room } from '@/locales/fr/room';
import { inbox } from '@/locales/fr/inbox';
import { notifications } from '@/locales/fr/notifications';
import { nav } from '@/locales/fr/nav';
import { legal } from '@/locales/fr/legal';
import { community } from '@/locales/fr/community';
import { admin } from '@/locales/fr/admin';
import { party } from '@/locales/fr/party';
import { globalQuestion } from '@/locales/fr/global';
import { streaks } from '@/locales/fr/streaks';
import { anonymous } from '@/locales/fr/anonymous';
import { tripot } from '@/locales/fr/tripot';
import { reactions } from '@/locales/fr/reactions';
import { said } from '@/locales/fr/said';
import { badges } from '@/locales/fr/badges';
import { wrapped } from '@/locales/fr/wrapped';
import { today } from '@/locales/fr/today';
import { intro } from '@/locales/fr/intro';
import { widget } from '@/locales/fr/widget';

export const fr = {
  common,
  vote,
  results,
  chat,
  moderation,
  celebration,
  questions,
  history,
  stats,
  packs,
  share,
  auth,
  security,
  errors,
  install,
  update,
  lock,
  profile,
  avatar,
  friends,
  room,
  inbox,
  notifications,
  nav,
  legal,
  community,
  admin,
  party,
  globalQuestion,
  streaks,
  anonymous,
  tripot,
  reactions,
  said,
  badges,
  wrapped,
  today,
  intro,
  widget,
};

/** The shape every language follows (strings widened, same keys). */
export type Messages = typeof fr;
