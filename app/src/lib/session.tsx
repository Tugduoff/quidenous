import { useQueryClient } from '@tanstack/react-query';
import { createContext, type PropsWithChildren, useContext, useState } from 'react';
import { forgetWidget } from '@/widget/task';
import { API_URL, ApiError, apiRoom, setApiRoom, setApiToken, setApiUrl } from '@/lib/api/client';
import { useMountEffect } from '@/lib/hooks/use-mount-effect';
import { useOnSignedOut } from '@/lib/hooks/use-on-signed-out';
import { auth, type SessionResult, joinLegacy, startLegacy, type LegacyEntry } from '@/lib/api/auth';
import { adoptAccountLang, loadLang, syncLang } from '@/lib/i18n';
import { introAtSignIn, introAtStart, introFinish, introForget } from '@/lib/api/intro';
import { clearRoomData } from '@/lib/api/query-client';
import { ROOM_KEY, SERVER_KEY, storage, TOKEN_KEY } from '@/lib/device/storage';
import { presetRoom } from '@/lib/device/launch-params';
import type { Member } from '@/lib/api/group';
import { flow, pendingJoinCode, setPendingCode } from '@/lib/auth-flow';
import { listRooms, joinRoom } from '@/lib/api/rooms';
import { getMe } from '@/lib/api/me';

const ROOMLESS_KEY = 'qdn.roomless';

type Session = {
  ready: boolean;
  me: Member | null;
  token: string | null;
  room: number | null;
  roomless: boolean;
  verifying: boolean;
  intro: boolean;
  endIntro: (skipped: boolean) => void;
  setRoom: (id: number | null) => Promise<void>;
  join: (code: string, name: string, server?: string) => Promise<void>;
  start: (name: string, room: string, server?: string) => Promise<void>;
  signIn: (res: SessionResult) => Promise<void>;
  doneVerifying: () => void;
  enterRoom: (id: number) => Promise<void>;
  noRoomLeft: () => Promise<void>;
  joinParty: (guestToken: string | null, roomId: number) => Promise<void>;
  setMe: (m: Member) => void;
  leave: () => Promise<void>;
  signOut: () => Promise<void>;
};

const SessionContext = createContext<Session | null>(null);

async function loadMe(): Promise<Member | 'no_room' | 'out' | 'offline'> {
  const found = await askMe();
  if (found === 'no_room') await storage.set(ROOMLESS_KEY, '1');
  else if (typeof found === 'object') await storage.remove(ROOMLESS_KEY);
  return found;
}

async function askMe(): Promise<Member | 'no_room' | 'out' | 'offline'> {
  if (apiRoom() == null && (await storage.get(ROOMLESS_KEY))) {
    const rooms = await listRooms().catch(() => null);
    if (Array.isArray(rooms) && !rooms.length) return 'no_room';
  }
  try {
    return await getMe();
  } catch (e) {
    if (!(e instanceof ApiError)) return 'offline';
    if (e.status === 403 && e.code === 'no_room') return 'no_room';
    if (e.status === 403 && apiRoom() != null) {
      setApiRoom(null);
      await storage.remove(ROOM_KEY);
      return askMe();
    }
    if (e.status === 401) return 'out';
    return 'offline';
  }
}

export function SessionProvider({ children }: PropsWithChildren) {
  const qc = useQueryClient();
  const [ready, setReady] = useState(false);
  const [token, setToken] = useState<string | null>(null);
  const [me, setMe] = useState<Member | null>(null);
  const [room, setRoomState] = useState<number | null>(null);
  const [roomless, setRoomless] = useState(false);
  const [verifying, setVerifying] = useState(false);
  const [intro, setIntro] = useState(false);

  const setRoom = async (id: number | null) => {
    setApiRoom(id);
    if (id == null) await storage.remove(ROOM_KEY);
    else await storage.set(ROOM_KEY, String(id));
    setRoomState(id);
    try {
      setMe(await getMe());
    } catch {}
  };

  useMountEffect(() => {
    (async () => {
      const server = await storage.get(SERVER_KEY);
      if (server) setApiUrl(server);
      const saved = await storage.get(TOKEN_KEY);
      await loadLang(!!saved);
      const savedRoom = Number(await storage.get(ROOM_KEY)) || null;
      const preset = presetRoom();
      if (saved) {
        setApiToken(saved);
        setApiRoom(preset ?? savedRoom);
        const [found, showIntro] = await Promise.all([loadMe(), introAtStart(qc)]);
        if (preset != null && typeof found === 'object' && apiRoom() === preset) await storage.set(ROOM_KEY, String(preset));
        if (found === 'out') {
          setApiToken(null);
          await storage.remove(TOKEN_KEY);
        } else {
          if (typeof found === 'object') setMe(found);
          if (found !== 'offline') syncLang();
          setRoomless(found === 'no_room');
          setRoomState(apiRoom());
          setIntro(showIntro);
          setToken(saved);
        }
      }
      setReady(true);
    })();
  });

  const join = (code: string, name: string, server?: string) => enter(() => joinLegacy(code, name), server);
  const start = (name: string, room: string, server?: string) => enter(() => startLegacy(name, room), server);

  const enter = async (call: () => Promise<LegacyEntry>, server?: string) => {
    if (server) setApiUrl(server);
    const res = await call();
    setApiRoom(res.room_id);
    setRoomState(res.room_id);
    await storage.set(ROOM_KEY, String(res.room_id));
    await storage.set(TOKEN_KEY, res.token);
    await storage.set(SERVER_KEY, API_URL);
    setApiToken(res.token);
    setToken(res.token);
    setMe(res.member);
    syncLang();
  };

  const signIn = async (res: SessionResult) => {
    clearRoomData(qc);
    setApiToken(res.token);
    setApiRoom(null);
    await storage.remove(ROOM_KEY);
    await storage.set(TOKEN_KEY, res.token);
    await storage.set(SERVER_KEY, API_URL);
    const introDue = introAtSignIn(qc);
    const code = pendingJoinCode();
    if (code) {
      try {
        const joined = await joinRoom(code);
        setPendingCode(null);
        setApiRoom(joined.id);
        await storage.set(ROOM_KEY, String(joined.id));
      } catch {}
    }
    const found = await loadMe();
    if (found === 'out') throw new ApiError(401, 'unauthorized');
    const showIntro = await introDue;
    adoptAccountLang();
    setMe(typeof found === 'object' ? found : null);
    setRoomless(found === 'no_room');
    setRoomState(apiRoom());
    flow.mailFailed = !!res.email_failed;
    setVerifying(!!res.email_sent || !!res.email_failed);
    setIntro(showIntro);
    setToken(res.token);
  };

  const enterRoom = async (id: number) => {
    await storage.remove(ROOMLESS_KEY);
    await setRoom(id);
    clearRoomData(qc);
    setPendingCode(null);
    setRoomless(false);
  };

  const noRoomLeft = async () => {
    setApiRoom(null);
    await storage.remove(ROOM_KEY);
    await storage.set(ROOMLESS_KEY, '1');
    setRoomState(null);
    setRoomless(true);
  };

  const joinParty = async (guestToken: string | null, roomId: number) => {
    if (guestToken) {
      setApiToken(guestToken);
      await storage.set(TOKEN_KEY, guestToken);
      await storage.set(SERVER_KEY, API_URL);
    }
    setApiRoom(roomId);
    await storage.set(ROOM_KEY, String(roomId));
    clearRoomData(qc);
    const found = await loadMe();
    if (found === 'out') throw new ApiError(401, 'unauthorized');
    setMe(typeof found === 'object' ? found : null);
    setRoomless(found === 'no_room');
    setRoomState(apiRoom());
    if (guestToken) {
      setVerifying(false);
      setToken(guestToken);
      syncLang();
    }
  };

  const leave = async () => {
    await storage.remove(TOKEN_KEY);
    await storage.remove(ROOM_KEY);
    await storage.remove(ROOMLESS_KEY);
    await introForget();
    forgetWidget().catch(() => {});
    setApiRoom(null);
    setRoomState(null);
    setApiToken(null);
    setToken(null);
    setMe(null);
    setRoomless(false);
    setVerifying(false);
    setIntro(false);
    qc.clear();
  };

  const signOut = async () => {
    await auth.logout().catch(() => {});
    await leave();
  };

  useOnSignedOut(() => void leave());

  return (
    <SessionContext.Provider
      value={{
        ready,
        me,
        token,
        room,
        roomless,
        verifying,
        intro,
        endIntro: (skipped: boolean) => {
          setIntro(false);
          void introFinish(skipped);
        },
        setRoom,
        join,
        start,
        signIn,
        doneVerifying: () => setVerifying(false),
        enterRoom,
        noRoomLeft,
        joinParty,
        setMe,
        leave,
        signOut,
      }}>
      {children}
    </SessionContext.Provider>
  );
}

export function useSession() {
  const s = useContext(SessionContext);
  if (!s) throw new Error('useSession outside SessionProvider');
  return s;
}
