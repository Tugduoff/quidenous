import { createContext, type PropsWithChildren, useContext, useEffect, useState } from 'react';

import { API_URL, api, ApiError, setApiRoom, setApiToken, setApiUrl, type Member } from './api';
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
  setRoom: (id: number | null) => Promise<void>;
  join: (code: string, name: string, server?: string) => Promise<void>;
  setMe: (m: Member) => void;
  leave: () => Promise<void>;
};

const SessionContext = createContext<Session | null>(null);

export function SessionProvider({ children }: PropsWithChildren) {
  const [ready, setReady] = useState(false);
  const [token, setToken] = useState<string | null>(null);
  const [me, setMe] = useState<Member | null>(null);
  const [room, setRoomState] = useState<number | null>(null);

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
      const savedRoom = Number(await storage.get(ROOM_KEY)) || null;
      if (saved) {
        setApiToken(saved);
        setApiRoom(savedRoom);
        setRoomState(savedRoom);
        try {
          setMe(await api<Member>('/me'));
          setToken(saved);
        } catch (e) {
          if (e instanceof ApiError && e.status === 403 && savedRoom != null) {
            // That room is gone or I left it: back to the server's choice.
            setApiRoom(null);
            setRoomState(null);
            await storage.remove(ROOM_KEY);
            try {
              setMe(await api<Member>('/me'));
              setToken(saved);
            } catch {}
          } else if (e instanceof ApiError && e.status === 401) await storage.remove(TOKEN_KEY);
          else setToken(saved); // offline: keep the session
        }
      }
      setReady(true);
    })();
  }, []);

  const join = async (code: string, name: string, server?: string) => {
    if (server) setApiUrl(server);
    const res = await api<{ token: string; member: Member; room_id: number }>('/join', { method: 'POST', json: { code, name } });
    setApiRoom(res.room_id);
    setRoomState(res.room_id);
    await storage.set(ROOM_KEY, String(res.room_id));
    await storage.set(TOKEN_KEY, res.token);
    await storage.set(SERVER_KEY, API_URL);
    setApiToken(res.token);
    setToken(res.token);
    setMe(res.member);
  };

  const leave = async () => {
    await storage.remove(TOKEN_KEY);
    await storage.remove(ROOM_KEY);
    setApiRoom(null);
    setRoomState(null);
    setApiToken(null);
    setToken(null);
    setMe(null);
  };

  return (
    <SessionContext.Provider value={{ ready, me, token, room, setRoom, join, setMe, leave }}>{children}</SessionContext.Provider>
  );
}

export function useSession() {
  const s = useContext(SessionContext);
  if (!s) throw new Error('useSession outside SessionProvider');
  return s;
}
