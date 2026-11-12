// Service worker: shows push notifications and opens the app on tap.
self.addEventListener('install', () => self.skipWaiting());
self.addEventListener('activate', (event) => event.waitUntil(self.clients.claim()));

self.addEventListener('push', (event) => {
  let data = {};
  try {
    data = event.data ? event.data.json() : {};
  } catch {
    data = { title: 'qui de nous ?', body: event.data ? event.data.text() : '' };
  }
  event.waitUntil(
    self.registration.showNotification(data.title || 'qui de nous ?', {
      body: data.body || '',
      tag: data.tag || 'round',
      renotify: true,
      icon: '/icon-192.png',
      badge: '/icon-192.png',
      data: { url: data.url || '/', room: data.room ?? null },
    }),
  );
});

self.addEventListener('notificationclick', (event) => {
  event.notification.close();
  // The url names the room (`/round/12?room=3`): the app opens in it. Added here if an older payload left it out.
  const target = new URL(event.notification.data?.url || '/', self.location.origin);
  const room = event.notification.data?.room;
  if (room != null && !target.searchParams.has('room')) target.searchParams.set('room', String(room));
  const url = target.href;
  event.waitUntil(
    self.clients.matchAll({ type: 'window', includeUncontrolled: true }).then((windows) => {
      const open = windows.find((w) => w.url.startsWith(self.location.origin));
      return open ? open.focus().then((w) => w.navigate(url)) : self.clients.openWindow(url);
    }),
  );
});
