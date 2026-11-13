import { useQueryClient } from '@tanstack/react-query';
import { createContext, type PropsWithChildren, useContext, useEffect, useState } from 'react';

import { API_URL, api, ApiError, apiRoom, presetRoom, setApiRoom, setApiToken, setApiUrl, setOnSignedOut, type Member } from './api';
import { auth, pendingJoinCode, setPendingCode, type SessionResult } from './auth';
import { deviceHeader } from './device';
import { adoptAccountLang, loadLang, syncLang } from './i18n';
import { storage } from './storage';

const TOKEN_KEY = 'qdn.token';
const SERVER_KEY = 'qdn.server';
const ROOM_KEY = 'qdn.room';

type Session = {
  ready: boolean;
  me: Member | null;
  token: string | null;
  /** The room on screen (null until known: the server then picks the last one used). */
  room: number | null;
  /** Signed in but in no room yet (`403 no_room`): join or create one first. */
  roomless: boolean;
  /** Just signed up and a code was emailed: the code screen comes first. */
  verifying: boolean;
  setRoom: (id: number | null) => Promise<void>;
  /** Legacy (token-only) entry: join with a code and a first name. */
  join: (code: string, name: string, server?: string) => Promise<void>;
  /** Legacy entry without a code: a new account with a new room I own. */
  start: (name: string, room: string, server?: string) => Promise<void>;
  /** A session from sign-in / sign-up / OAuth / reset. Joins the room of an invite opened before. */
  signIn: (res: SessionResult) => Promise<void>;
  doneVerifying: () => void;
  /** First room of a roomless account (joined or created). */
  enterRoom: (id: number) => Promise<void>;
  setMe: (m: Member) => void;
  /** Forget the session on this device only. */
  leave: () => Promise<void>;
  /** Sign out: the server drops the token too. */
  signOut: () => Promise<void>;
};

const SessionContext = createContext<Session | null>(null);

/** Who I am in the current room, or why not. A room I left is forgotten and the server picks another. */
async function loadMe(): Promise<Member | 'no_room' | 'out' | 'offline'> {
  try {
    return await api<Member>('/me');
  } catch (e) {
    if (!(e instanceof ApiError)) return 'offline';
    if (e.status === 403 && e.code === 'no_room') return 'no_room';
    if (e.status === 403 && apiRoom() != null) {
      setApiRoom(null);
      await storage.remove(ROOM_KEY);
      return loadMe();
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

  const setRoom = async (id: number | null) => {
    setApiRoom(id);
    if (id == null) await storage.remove(ROOM_KEY);
    else await storage.set(ROOM_KEY, String(id));
    setRoomState(id);
    // My seat (name, photo) can differ per room.
    try {
      setMe(await api<Member>('/me'));
    } catch {}
  };

  useEffect(() => {
    (async () => {
      const server = await storage.get(SERVER_KEY);
      if (server) setApiUrl(server);
      const saved = await storage.get(TOKEN_KEY);
      await loadLang(!!saved);
      const savedRoom = Number(await storage.get(ROOM_KEY)) || null;
      // A tapped notification (web) names its room: opened in it from the first request, nothing to refetch.
      const preset = presetRoom();
      if (saved) {
        setApiToken(saved);
        setApiRoom(preset ?? savedRoom);
        const found = await loadMe();
        if (preset != null && typeof found === 'object' && apiRoom() === preset) await storage.set(ROOM_KEY, String(preset));
        if (found === 'out') {
          setApiToken(null);
          await storage.remove(TOKEN_KEY);
        } else {
          if (typeof found === 'object') setMe(found);
          if (found !== 'offline') syncLang();
          setRoomless(found === 'no_room');
          setRoomState(apiRoom());
          setToken(saved); // offline too: keep the session
        }
      }
      setReady(true);
    })();
  }, []);

  const join = (code: string, name: string, server?: string) => enter('/join', { code, name }, server);
  const start = (name: string, room: string, server?: string) => enter('/start', { name, room }, server);

  const enter = async (path: string, json: Record<string, string>, server?: string) => {
    if (server) setApiUrl(server);
    const res = await api<{ token: string; member: Member; room_id: number }>(path, { method: 'POST', json, headers: deviceHeader() });
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
    qc.clear();
    setApiToken(res.token);
    setApiRoom(null);
    await storage.remove(ROOM_KEY);
    await storage.set(TOKEN_KEY, res.token);
    await storage.set(SERVER_KEY, API_URL);
    const code = pendingJoinCode();
    if (code) {
      try {
        const joined = await api<{ id: number }>('/rooms/join', { method: 'POST', json: { code } });
        setPendingCode(null);
        setApiRoom(joined.id);
        await storage.set(ROOM_KEY, String(joined.id));
      } catch {} // kept: it fills the code field of the room screen
    }
    const found = await loadMe();
    if (found === 'out') throw new ApiError(401, 'unauthorized');
    adoptAccountLang();
    // Set together: the screens switch once, straight to the right one.
    setMe(typeof found === 'object' ? found : null);
    setRoomless(found === 'no_room');
    setRoomState(apiRoom());
    setVerifying(!!res.email_sent);
    setToken(res.token);
  };

  const enterRoom = async (id: number) => {
    await setRoom(id);
    qc.clear();
    setPendingCode(null);
    setRoomless(false);
  };

  const leave = async () => {
    await storage.remove(TOKEN_KEY);
    await storage.remove(ROOM_KEY);
    setApiRoom(null);
    setRoomState(null);
    setApiToken(null);
    setToken(null);
    setMe(null);
    setRoomless(false);
    setVerifying(false);
    qc.clear();
  };

  const signOut = async () => {
    // An older server has no /auth/logout: the token is then only forgotten here.
    await auth.logout().catch(() => {});
    await leave();
  };

  // Signed out from another device (or the token is gone): back to the welcome screen.
  useEffect(() => {
    setOnSignedOut(() => void leave());
    return () => setOnSignedOut(null);
  });

  return (
    <SessionContext.Provider
      value={{
        ready,
        me,
        token,
        room,
        roomless,
        verifying,
        setRoom,
        join,
        start,
        signIn,
        doneVerifying: () => setVerifying(false),
        enterRoom,
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
