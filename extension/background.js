// Service worker: central hub. Receives codes from a code source (either the
// webmail tab poller, or the native-messaging daemon), persists the latest one,
// raises a browser notification, and pushes it to the active tab's toast.
var api = (typeof browser !== 'undefined') ? browser : chrome;

var NATIVE_HOST = 'com.mibs.otp_relay';
var POLLER_ID = 'webmail-poller';
var DEFAULTS = {
  source: 'tab',              // 'tab' | 'native'
  origin: '',                 // webmail origin, e.g. https://mail.example.com
  codeTtlMs: 300000,
  notify: true
};

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
  if (!msg || !msg.code) return;
  var rec = { code: msg.code, meta: msg.meta || {}, ts: Date.now() };
  await api.storage.local.set({ otpLatest: rec });

  if (cfg.notify) {
    try {
      api.notifications.create('otp-' + rec.ts, {
        type: 'basic',
        iconUrl: api.runtime.getURL('icons/icon-128.png'),
        title: 'Verification code: ' + rec.code,
        message: (rec.meta.from && rec.meta.from.name ? rec.meta.from.name + ' — ' : '') +
                 (rec.meta.subject || 'New code received'),
        priority: 2
      });
    } catch (e) { /* notifications may be unavailable */ }
  }

  try {
    var tabs = await api.tabs.query({ active: true, currentWindow: true });
    for (var i = 0; i < tabs.length; i++) {
      if (tabs[i].id != null) {
        api.tabs.sendMessage(tabs[i].id, {
          type: 'otp:show', code: rec.code, meta: rec.meta, ts: rec.ts
        }).catch(function () {});
      }
    }
  } catch (e) { /* no receiver */ }
}

// --- code source: webmail tab poller (dynamic registration) ----------------
async function syncPoller() {
  var cfg = await getCfg();
  try { await api.scripting.unregisterContentScripts({ ids: [POLLER_ID] }); } catch (e) {}
  if (cfg.source !== 'tab' || !cfg.origin) return;
  var pattern = cfg.origin.replace(/\/+$/, '') + '/*';
  var granted = false;
  try { granted = await api.permissions.contains({ origins: [pattern] }); } catch (e) {}
  if (!granted) return; // options page requests the permission on the Save gesture
  try {
    await api.scripting.registerContentScripts([{
      id: POLLER_ID,
      matches: [pattern],
      js: ['otp-detect.js', 'webmail-poller.js'],
      runAt: 'document_idle',
      persistAcrossSessions: true
    }]);
  } catch (e) { /* pattern may be invalid */ }
}

// --- code source: native-messaging daemon ----------------------------------
var nativePort = null;

function connectNative() {
  if (nativePort) return;
  try {
    nativePort = api.runtime.connectNative(NATIVE_HOST);
  } catch (e) { nativePort = null; return; }
  nativePort.onMessage.addListener(function (msg) {
    // Spike stub sends {type:'hello'}; real daemon sends {type:'code', code, meta}.
    if (msg && msg.type === 'code' && msg.code) handleNew({ code: msg.code, meta: msg.meta });
  });
  nativePort.onDisconnect.addListener(function () {
    nativePort = null;
    // Reconnect if we are still in native mode (the daemon/bridge may have exited).
    getCfg().then(function (cfg) { if (cfg.source === 'native') scheduleNativeReconnect(); });
  });
}

function disconnectNative() {
  if (nativePort) { try { nativePort.disconnect(); } catch (e) {} nativePort = null; }
}

function scheduleNativeReconnect() {
  // Keep-alive: an alarm re-wakes the SW and reconnects if the port dropped.
  try { api.alarms.create('native-keepalive', { periodInMinutes: 0.5 }); } catch (e) {}
}

async function applySource() {
  var cfg = await getCfg();
  if (cfg.source === 'native') { disconnectNative(); connectNative(); scheduleNativeReconnect(); }
  else { disconnectNative(); try { api.alarms.clear('native-keepalive'); } catch (e) {} }
  await syncPoller();
}

api.alarms && api.alarms.onAlarm.addListener(function (a) {
  if (a && a.name === 'native-keepalive') {
    getCfg().then(function (cfg) { if (cfg.source === 'native' && !nativePort) connectNative(); });
  }
});

// --- wiring ----------------------------------------------------------------
api.runtime.onMessage.addListener(function (msg, sender, sendResponse) {
  if (!msg) return;
  if (msg.type === 'otp:new') { handleNew(msg); return; }
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
  if (msg.type === 'otp:reconfigure') { applySource(); return; }
});

api.storage.onChanged.addListener(function (changes, area) {
  if (area === 'local' && changes.otpConfig) applySource();
});

api.notifications.onClicked && api.notifications.onClicked.addListener(function (id) {
  api.notifications.clear(id);
});

// Initial setup on SW start.
applySource();
