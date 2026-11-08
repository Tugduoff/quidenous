import { createContext, type PropsWithChildren, useContext, useEffect, useState } from 'react';

import { API_URL, api, ApiError, setApiToken, setApiUrl, type Member } from './api';
import { storage } from './storage';

const TOKEN_KEY = 'qdn.token';
const SERVER_KEY = 'qdn.server';

type Session = {
  ready: boolean;
  me: Member | null;
  token: string | null;
  join: (code: string, name: string, server?: string) => Promise<void>;
  setMe: (m: Member) => void;
  leave: () => Promise<void>;
};

const SessionContext = createContext<Session | null>(null);

export function SessionProvider({ children }: PropsWithChildren) {
  const [ready, setReady] = useState(false);
  const [token, setToken] = useState<string | null>(null);
  const [me, setMe] = useState<Member | null>(null);

  useEffect(() => {
    (async () => {
      const server = await storage.get(SERVER_KEY);
      if (server) setApiUrl(server);
      const saved = await storage.get(TOKEN_KEY);
      if (saved) {
        setApiToken(saved);
        try {
          setMe(await api<Member>('/me'));
          setToken(saved);
        } catch (e) {
          if (e instanceof ApiError && e.status === 401) await storage.remove(TOKEN_KEY);
          else setToken(saved); // offline: keep the session
        }
      }
      setReady(true);
    })();
  }, []);

  const join = async (code: string, name: string, server?: string) => {
    if (server) setApiUrl(server);
    const res = await api<{ token: string; member: Member }>('/join', { method: 'POST', json: { code, name } });
    await storage.set(TOKEN_KEY, res.token);
    await storage.set(SERVER_KEY, API_URL);
    setApiToken(res.token);
    setToken(res.token);
    setMe(res.member);
  };

  const leave = async () => {
    await storage.remove(TOKEN_KEY);
    setApiToken(null);
    setToken(null);
    setMe(null);
  };

  return (
    <SessionContext.Provider value={{ ready, me, token, join, setMe, leave }}>{children}</SessionContext.Provider>
  );
}

export function useSession() {
  const s = useContext(SessionContext);
  if (!s) throw new Error('useSession outside SessionProvider');
  return s;
}
