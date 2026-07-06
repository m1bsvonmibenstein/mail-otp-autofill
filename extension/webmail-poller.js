// Runs inside the SOGo webmail tab (same-origin -> MCSESSID auth works).
// Polls the inbox, extracts verification codes from newly-arrived mail via
// SOGo's viewplain endpoint, restores unread state, and forwards codes to the
// background service worker. Requires a webmail tab to be open (MCSESSID is
// SameSite=Lax, so a cross-site background fetch cannot authenticate).
(function () {
  'use strict';
  var api = (typeof browser !== 'undefined') ? browser : chrome;
  var findCode = (self.OTP && self.OTP.findCode) || function () { return null; };
  var findLink = (self.OTP && self.OTP.findLink) || function () { return null; };
  var findLinkInHtml = (self.OTP && self.OTP.findLinkInHtml) || function () { return null; };

  // Scheme+host only; base() appends '/SOGo/so/...', so a stored path would 404.
  function normOrigin(s) {
    s = (s || '').trim();
    if (!s) return '';
    if (!/^[a-z][a-z0-9+.-]*:\/\//i.test(s)) s = 'https://' + s;
    try { return new URL(s).origin; } catch (e) { return s.replace(/([^/])\/.*$/, '$1').replace(/\/+$/, ''); }
  }

  var DEFAULTS = {
    origin: location.origin,
    login: '',
    folder: '0/folderINBOX',
    pollIntervalMs: 20000,
    processMax: 6,
    restoreUnread: true,
    enabled: true
  };

  var cfg = null, lastMaxUid = null, timer = null;
  var seen = Object.create(null);

  function base() {
    return cfg.origin + '/SOGo/so/' + cfg.login + '/Mail/' + cfg.folder;
  }

  function detectLogin() {
    var m = location.pathname.match(/\/SOGo\/so\/([^\/]+)\//);
    return m ? decodeURIComponent(m[1]) : '';
  }

  async function listMessages() {
    var r = await fetch(base() + '/view', {
      method: 'POST',
      credentials: 'include',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ sortingAttributes: { sort: 'date', asc: false } })
    });
    if (!r.ok) return null;
    var j = await r.json();
    if (!j.headers || !j.headers[0]) return null;
    var cols = j.headers[0];
    var iUid = cols.indexOf('uid'), iSub = cols.indexOf('Subject');
    var iRead = cols.indexOf('isRead'), iFrom = cols.indexOf('From');
    return j.headers.slice(1).map(function (row) {
      var from = (iFrom >= 0 && row[iFrom] && row[iFrom][0]) ? row[iFrom][0] : null;
      return {
        uid: row[iUid],
        subject: iSub >= 0 ? (row[iSub] || '') : '',
        isRead: iRead >= 0 ? row[iRead] : 1,
        from: from
      };
    });
  }

  async function bodyText(uid) {
    var r = await fetch(base() + '/' + uid + '/viewplain', { credentials: 'include' });
    if (!r.ok) return '';
    var j = await r.json();
    return (j && j.content) || '';
  }

  // HTML body via SOGo's /view (JSON with a nested parts tree). Collect the
  // string `content` fields that actually hold links; skip base64 image parts.
  async function bodyHtml(uid) {
    var r = await fetch(base() + '/' + uid + '/view', { credentials: 'include' });
    if (!r.ok) return '';
    var j = await r.json();
    var out = [];
    (function walk(node) {
      if (!node) return;
      if (Array.isArray(node)) { for (var i = 0; i < node.length; i++) walk(node[i]); return; }
      if (typeof node !== 'object') return;
      if (typeof node.content === 'string' && /https?:|href=/i.test(node.content)) out.push(node.content);
      for (var k in node) { var v = node[k]; if (v && typeof v === 'object') walk(v); }
    })(j && j.parts);
    return out.join('\n');
  }

  function markUnread(uid) {
    fetch(base() + '/' + uid + '/markMessageUnread', { credentials: 'include' }).catch(function () {});
  }

  function persistWatermark() {
    api.storage.local.set({ otpWatermark: lastMaxUid }).catch(function () {});
  }

  async function pollOnce() {
    if (!cfg.enabled || !cfg.login) return;
    var msgs;
    try { msgs = await listMessages(); } catch (e) { return; }
    if (!msgs || !msgs.length) return;

    var maxUid = msgs.reduce(function (a, m) { return m.uid > a ? m.uid : a; }, 0);
    // baseline on first ever run only; a persisted watermark lets us emit mail
    // that arrived while the tab was closed without replaying old mail.
    if (lastMaxUid === null) { lastMaxUid = maxUid; persistWatermark(); return; }

    var fresh = msgs
      .filter(function (m) { return m.uid > lastMaxUid && !seen[m.uid]; })
      .sort(function (a, b) { return b.uid - a.uid; })
      .slice(0, cfg.processMax);
    lastMaxUid = Math.max(lastMaxUid, maxUid);
    persistWatermark();

    for (var i = 0; i < fresh.length; i++) {
      var m = fresh[i];
      seen[m.uid] = true;
      var code = findCode(m.subject);
      var link = null;
      if (!code) {
        try {
          var txt = await bodyText(m.uid);
          code = findCode(txt);
          // A message is normally a code OR a magic link, so only look for a
          // link once the code path comes up empty.
          if (!code) link = findLink(txt);
          // HTML-only mail: the raw URL never survives plaintext conversion, so
          // fetch the HTML body and pull the sign-in anchor's href.
          if (!code && !link) {
            link = findLinkInHtml(await bodyHtml(m.uid));
          }
          if (cfg.restoreUnread && m.isRead === 0) markUnread(m.uid);
        } catch (e) { /* ignore this message */ }
      }
      if (code || link) {
        api.runtime.sendMessage({
          type: 'otp:new',
          code: code || null,
          link: link || null,
          meta: { uid: m.uid, subject: m.subject, from: m.from, ts: Date.now() }
        });
      }
    }
  }

  function start() {
    if (!cfg.login) cfg.login = detectLogin();
    if (!cfg.login) { console.warn('[OTP] no login detected; set it in options'); return; }
    if (timer) clearInterval(timer);
    pollOnce();
    timer = setInterval(pollOnce, Math.max(10000, cfg.pollIntervalMs));
  }

  api.storage.local.get(['otpConfig', 'otpWatermark']).then(function (o) {
    cfg = Object.assign({}, DEFAULTS, (o && o.otpConfig) || {});
    cfg.origin = normOrigin(cfg.origin) || location.origin;
    if (typeof o.otpWatermark === 'number') lastMaxUid = o.otpWatermark;
    start();
  });

  // Re-read config when the user saves options.
  api.storage.onChanged.addListener(function (changes, area) {
    if (area === 'local' && changes.otpConfig) {
      cfg = Object.assign({}, DEFAULTS, changes.otpConfig.newValue || {});
      cfg.origin = normOrigin(cfg.origin) || location.origin;
      start();
    }
  });
})();
