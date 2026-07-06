// Service worker: central hub. Receives codes from a code source (either the
// webmail tab poller, or the native-messaging daemon), persists the latest one,
// raises a browser notification, and pushes it to the active tab's toast.
var api = (typeof browser !== 'undefined') ? browser : chrome;

var NATIVE_HOST = 'com.mibs.otp_relay';
var POLLER_ID = 'webmail-poller';

// Scheme+host only. Feeds the content-script match pattern; a pasted path (esp.
// a lowercase '/sogo' that won't match the real '/SOGo') would otherwise stop
// the poller from ever injecting.
function normOrigin(s) {
  s = (s || '').trim();
  if (!s) return '';
  if (!/^[a-z][a-z0-9+.-]*:\/\//i.test(s)) s = 'https://' + s;
  try { return new URL(s).origin; } catch (e) { return s.replace(/([^/])\/.*$/, '$1').replace(/\/+$/, ''); }
}
var DEFAULTS = {
  source: 'tab',              // 'tab' | 'native'
  origin: '',                 // webmail origin, e.g. https://mail.example.com
  codeTtlMs: 300000,
  notify: true
};

// --- diagnostics (spike): ring-buffer to storage so options can display it ---
function dbg(m) {
  try { console.log('[OTP]', m); } catch (e) {}
  api.storage.local.get('otpDbg').then(function (o) {
    var a = (o && o.otpDbg) || [];
    a.push(new Date().toISOString().slice(11, 23) + ' ' + m);
    if (a.length > 40) a = a.slice(-40);
    api.storage.local.set({ otpDbg: a });
  }).catch(function () {});
}

async function getCfg() {
  var o = await api.storage.local.get('otpConfig');
  return Object.assign({}, DEFAULTS, (o && o.otpConfig) || {});
}

async function getLatest() {
  var cfg = await getCfg();
  var o = await api.storage.local.get('otpLatest');
  var rec = o && o.otpLatest;
  if (rec && (Date.now() - rec.ts) < cfg.codeTtlMs) return rec;
  return null;
}

async function handleNew(msg) {
  var cfg = await getCfg();
  if (!msg || (!msg.code && !msg.link)) return;
  var rec = { code: msg.code || null, link: msg.link || null, meta: msg.meta || {}, ts: Date.now() };
  await api.storage.local.set({ otpLatest: rec });

  if (cfg.notify) {
    var from = rec.meta.from && rec.meta.from.name ? rec.meta.from.name + ' - ' : '';
    var title = rec.link ? 'Sign-in link' : 'Verification code: ' + rec.code;
    var body = rec.link
      ? from + rec.link.host + (rec.meta.subject ? ' - ' + rec.meta.subject : '')
      : from + (rec.meta.subject || 'New code received');
    try {
      api.notifications.create('otp-' + rec.ts, {
        type: 'basic',
        iconUrl: api.runtime.getURL('icons/icon-128.png'),
        title: title,
        message: body,
        priority: 2
      });
    } catch (e) { /* notifications may be unavailable */ }
  }

  try {
    var tabs = await api.tabs.query({ active: true, currentWindow: true });
    for (var i = 0; i < tabs.length; i++) {
      if (tabs[i].id != null) {
        api.tabs.sendMessage(tabs[i].id, {
          type: 'otp:show', code: rec.code, link: rec.link, meta: rec.meta, ts: rec.ts
        }).catch(function () {});
      }
    }
  } catch (e) { /* no receiver */ }
}

// Open a magic link in a new tab. Re-validate https here since the content
// script (source of the URL) is the untrusted side of this boundary.
function openLink(url) {
  if (typeof url !== 'string' || !/^https:\/\//i.test(url)) return;
  try { api.tabs.create({ url: url }); } catch (e) { dbg('openLink failed: ' + e.message); }
}

// --- code source: webmail tab poller (dynamic registration) ----------------
async function syncPoller() {
  var cfg = await getCfg();
  try { await api.scripting.unregisterContentScripts({ ids: [POLLER_ID] }); } catch (e) {}
  if (cfg.source !== 'tab' || !cfg.origin) return;
  // Collapse to scheme+host so a pasted '/SOGo' path (or lowercase '/sogo',
  // which won't match the case-sensitive real path) can't break the pattern.
  var origin = normOrigin(cfg.origin);
  if (!origin) { dbg('poller: bad origin ' + cfg.origin); return; }
  var pattern = origin + '/*';
  var granted = false;
  try { granted = await api.permissions.contains({ origins: [pattern] }); } catch (e) {}
  if (!granted) { dbg('poller: no permission for ' + pattern); return; }
  try {
    await api.scripting.registerContentScripts([{
      id: POLLER_ID,
      matches: [pattern],
      js: ['otp-detect.js', 'webmail-poller.js'],
      runAt: 'document_idle',
      persistAcrossSessions: true
    }]);
    dbg('poller registered on ' + pattern);
  } catch (e) { dbg('poller register error: ' + e.message); }
}

// --- code source: native-messaging daemon ----------------------------------
var nativePort = null;

function connectNative() {
  if (nativePort) { dbg('connectNative: already connected'); return; }
  dbg('connectNative: attempting');
  try {
    nativePort = api.runtime.connectNative(NATIVE_HOST);
  } catch (e) { nativePort = null; dbg('connectNative threw: ' + e.message); return; }
  dbg('connectNative: port created');
  nativePort.onMessage.addListener(function (msg) {
    dbg('native recv: ' + JSON.stringify(msg));
    if (msg && msg.type === 'code' && msg.code) handleNew({ code: msg.code, meta: msg.meta });
    if (msg && msg.type === 'link' && msg.url) {
      handleNew({ link: { url: msg.url, host: msg.host }, meta: msg.meta });
    }
  });
  nativePort.onDisconnect.addListener(function () {
    var err = (api.runtime.lastError && api.runtime.lastError.message) || 'no error';
    nativePort = null;
    dbg('native disconnect: ' + err);
    getCfg().then(function (cfg) { if (cfg.source === 'native') scheduleNativeReconnect(); });
  });
}

function disconnectNative() {
  if (nativePort) { try { nativePort.disconnect(); } catch (e) {} nativePort = null; }
}

function scheduleNativeReconnect() {
  try { api.alarms.create('native-keepalive', { periodInMinutes: 0.5 }); } catch (e) {}
}

async function applySource() {
  var cfg = await getCfg();
  dbg('applySource: source=' + cfg.source);
  if (cfg.source === 'native') { disconnectNative(); connectNative(); scheduleNativeReconnect(); }
  else { disconnectNative(); try { api.alarms.clear('native-keepalive'); } catch (e) {} }
  await syncPoller();
}

api.alarms && api.alarms.onAlarm.addListener(function (a) {
  if (a && a.name === 'native-keepalive') {
    dbg('alarm: keepalive (port=' + (!!nativePort) + ')');
    getCfg().then(function (cfg) { if (cfg.source === 'native' && !nativePort) connectNative(); });
  }
});

// --- wiring ----------------------------------------------------------------
api.runtime.onMessage.addListener(function (msg, sender, sendResponse) {
  if (!msg) return;
  if (msg.type === 'otp:new') { handleNew(msg); return; }
  if (msg.type === 'otp:open') { openLink(msg.url); return; }
  if (msg.type === 'otp:getLatest') { getLatest().then(sendResponse); return true; }
  if (msg.type === 'otp:dismiss') {
    api.storage.local.get('otpLatest').then(function (o) {
      var rec = o && o.otpLatest;
      if (rec) {
        rec.dismissed = true;
        api.storage.local.set({ otpLatest: rec });
        if (nativePort) { try { nativePort.postMessage({ type: 'used' }); } catch (e) {} }
      }
    });
    return;
  }
  if (msg.type === 'otp:reconfigure') { dbg('reconfigure msg'); applySource(); return; }
});

api.storage.onChanged.addListener(function (changes, area) {
  if (area === 'local' && changes.otpConfig) { dbg('storage: otpConfig changed'); applySource(); }
});

api.notifications.onClicked && api.notifications.onClicked.addListener(function (id) {
  api.notifications.clear(id);
});

// Initial setup on SW start.
dbg('SW start');
applySource();
