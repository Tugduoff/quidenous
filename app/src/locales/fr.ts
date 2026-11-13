// French: the source of truth. Every key here must exist in en.ts (tsc checks it). See docs/I18N.md.
import { common } from './fr/common';
import { vote } from './fr/vote';
import { results } from './fr/results';
import { chat } from './fr/chat';
import { moderation } from './fr/moderation';
import { celebration } from './fr/celebration';
import { questions } from './fr/questions';
import { history } from './fr/history';
import { stats } from './fr/stats';
import { packs } from './fr/packs';
import { share } from './fr/share';
import { auth } from './fr/auth';
import { security } from './fr/security';
import { errors } from './fr/errors';
import { install } from './fr/install';
import { update } from './fr/update';
import { lock } from './fr/lock';
import { profile } from './fr/profile';
import { avatar } from './fr/avatar';
import { friends } from './fr/friends';
import { room } from './fr/room';
import { inbox } from './fr/inbox';
import { notifications } from './fr/notifications';
import { nav } from './fr/nav';
import { legal } from './fr/legal';
import { community } from './fr/community';
import { admin } from './fr/admin';

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
};

/** The shape every language follows (strings widened, same keys). */
export type Messages = typeof fr;
