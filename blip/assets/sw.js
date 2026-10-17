var cacheName = 'blip';
var filesToCache = [
  './',
  './index.html',
  './blip.js',
  './blip_bg.wasm',
];

/* The app's files as absolute URLs. Trunk doesn't hash the file names
   (filehash = false), so a new version has the same URLs as the old one. */
var appUrls = filesToCache.map(function (file) {
  return new URL(file, self.registration.scope).href;
});

/* Cache all of the app's content for offline use. `cache: 'reload'` bypasses
   the browser's HTTP cache, which could still hold the previous version. */
self.addEventListener('install', function (e) {
  e.waitUntil(
    caches.open(cacheName).then(function (cache) {
      return cache.addAll(appUrls.map(function (url) {
        return new Request(url, { cache: 'reload' });
      }));
    })
  );
  /* Take over from an old service worker right away instead of waiting until
     every tab of the app is closed. */
  self.skipWaiting();
});

/* Delete caches of other (older) service workers and control the open pages. */
self.addEventListener('activate', function (e) {
  e.waitUntil(
    caches.keys().then(function (names) {
      return Promise.all(names.filter(function (name) {
        return name !== cacheName;
      }).map(function (name) {
        return caches.delete(name);
      }));
    }).then(function () {
      return self.clients.claim();
    })
  );
});

/* Network first: always load the newest version and refresh the cache with it,
   and serve the cached content only when offline. Serving the cache first would
   keep an old version forever, as the URLs never change. Loading blip.js and
   blip_bg.wasm the same way also keeps the two consistent with each other.
   Everything else (e.g. the docs under docs/) goes to the network untouched. */
self.addEventListener('fetch', function (e) {
  if (e.request.method !== 'GET') {
    return;
  }
  var url = new URL(e.request.url);
  url.search = '';
  url.hash = '';
  if (appUrls.indexOf(url.href) === -1) {
    return;
  }
  e.respondWith(
    fetch(e.request, { cache: 'no-cache' }).then(function (response) {
      if (response.ok) {
        var copy = response.clone();
        e.waitUntil(caches.open(cacheName).then(function (cache) {
          return cache.put(url.href, copy);
        }));
      }
      return response;
    }).catch(function (error) {
      return caches.match(url.href).then(function (response) {
        return response || Promise.reject(error);
      });
    })
  );
});
