// Runs on every site. Shows a top-right toast with the latest verification code
// and offers Copy + Autofill. Autofill uses the native value setter so
// React/Vue-controlled inputs accept the value, and handles split OTP boxes.
(function () {
  'use strict';
  var api = (typeof browser !== 'undefined') ? browser : chrome;

  var host = null, shadow = null, hideTimer = null;
  var VISIBLE_MS = 60000;

  // ---- autofill ------------------------------------------------------------
  function isVisible(el) {
    if (!el || el.disabled || el.readOnly) return false;
    if (el.offsetParent === null) return false;
    var r = el.getBoundingClientRect();
    return r.width > 0 && r.height > 0;
  }

  function setNativeValue(el, value) {
    var proto = Object.getPrototypeOf(el);
    var desc = Object.getOwnPropertyDescriptor(proto, 'value') ||
               Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value');
    if (desc && desc.set) desc.set.call(el, value);
    else el.value = value;
    el.dispatchEvent(new Event('input', { bubbles: true }));
    el.dispatchEvent(new Event('change', { bubbles: true }));
  }

  function findTargets() {
    var inputs = Array.prototype.slice.call(document.querySelectorAll('input')).filter(isVisible);
    // 1) split single-char boxes (very common OTP pattern)
    var oneChar = inputs.filter(function (i) {
      var t = (i.type || 'text').toLowerCase();
      return i.maxLength === 1 && /^(text|tel|number|password)$/.test(t);
    });
    if (oneChar.length >= 4 && oneChar.length <= 8) return { kind: 'split', els: oneChar };
    // 2) explicit one-time-code field
    var el = document.querySelector('input[autocomplete="one-time-code"]');
    if (el && isVisible(el)) return { kind: 'single', els: [el] };
    // 3) heuristic name/id/placeholder/aria match
    el = inputs.find(function (i) {
      var hay = ((i.name || '') + ' ' + (i.id || '') + ' ' + (i.placeholder || '') + ' ' +
                 (i.getAttribute('aria-label') || '')).toLowerCase();
      return /otp|code|pin|token|verif|2fa|mfa|one.?time|passcode/.test(hay);
    });
    if (el) return { kind: 'single', els: [el] };
    // 4) fallback: focused input
    var act = document.activeElement;
    if (act && act.tagName === 'INPUT' && isVisible(act)) return { kind: 'single', els: [act] };
    return null;
  }

  function autofill(code) {
    var t = findTargets();
    if (!t) return { ok: false, msg: 'No code field found on this page' };
    if (t.kind === 'split') {
      var chars = code.split('');
      for (var i = 0; i < t.els.length && i < chars.length; i++) {
        t.els[i].focus();
        setNativeValue(t.els[i], chars[i]);
      }
    } else {
      t.els[0].focus();
      setNativeValue(t.els[0], code);
    }
    return { ok: true };
  }

  function copy(code) {
    if (navigator.clipboard && navigator.clipboard.writeText) return navigator.clipboard.writeText(code);
    var ta = document.createElement('textarea');
    ta.value = code; ta.style.position = 'fixed'; ta.style.opacity = '0';
    document.body.appendChild(ta); ta.select();
    try { document.execCommand('copy'); } catch (e) {}
    document.body.removeChild(ta);
    return Promise.resolve();
  }

  // ---- toast UI (shadow DOM to resist page CSS) ----------------------------
  function ensureHost() {
    if (host) return;
    host = document.createElement('div');
    host.id = 'mc-otp-host';
    host.style.cssText = 'all:initial;position:fixed;top:16px;right:16px;z-index:2147483647;';
    shadow = host.attachShadow({ mode: 'open' });
    document.documentElement.appendChild(host);
    var style = document.createElement('style');
    style.textContent = shadowStyle();
    shadow.appendChild(style);
  }

  function showToast(code, meta) {
    ensureHost();
    if (hideTimer) clearTimeout(hideTimer);
    var from = (meta && meta.from) ? (meta.from.name || meta.from.email || '') : '';
    var subj = [from, (meta && meta.subject) ? String(meta.subject) : '']
      .filter(Boolean).join(' — ');
    var wrap = document.createElement('div');
    wrap.className = 'card';
    wrap.innerHTML =
      '<div class="hdr"><span class="ttl">Verification code</span><span class="x">&times;</span></div>' +
      '<div class="code"></div>' +
      (subj ? '<div class="sub"></div>' : '') +
      '<div class="row"><button class="copy">Copy</button><button class="fill">Autofill</button></div>' +
      '<div class="msg"></div>';
    wrap.querySelector('.code').textContent = code;
    if (subj) wrap.querySelector('.sub').textContent = subj;

    var msgEl = wrap.querySelector('.msg');
    function flash(txt) { msgEl.textContent = txt; }

    wrap.querySelector('.x').addEventListener('click', function () { dismiss(); hide(); });
    wrap.querySelector('.copy').addEventListener('click', function () {
      copy(code).then(function () { flash('Copied ' + code); dismiss(); });
    });
    wrap.querySelector('.fill').addEventListener('click', function () {
      var res = autofill(code);
      flash(res.ok ? 'Filled ✓' : res.msg);
      if (res.ok) dismiss();
    });

    var prev = shadow.querySelector('.card');
    if (prev) prev.remove();
    shadow.appendChild(wrap);
    host.style.display = 'block';
    hideTimer = setTimeout(hide, VISIBLE_MS);
  }

  // Single source for the shadow-root CSS, injected once in ensureHost().
  function shadowStyle() {
    return [
      ':host{all:initial}',
      '.card{font:400 14px/1.4 system-ui,Segoe UI,Roboto,sans-serif;background:#1f2430;color:#fff;',
      'border-radius:12px;box-shadow:0 8px 28px rgba(0,0,0,.45);padding:14px 16px;min-width:230px;',
      'max-width:320px;border:1px solid rgba(255,255,255,.08)}',
      '.hdr{display:flex;align-items:center;justify-content:space-between;margin-bottom:6px}',
      '.ttl{font-size:11px;letter-spacing:.6px;text-transform:uppercase;opacity:.65}',
      '.x{cursor:pointer;opacity:.6;font-size:16px;line-height:1;padding:2px 4px}',
      '.x:hover{opacity:1}',
      '.code{font:700 26px/1 ui-monospace,SFMono-Regular,Menlo,monospace;letter-spacing:3px;margin:4px 0 10px}',
      '.sub{font-size:12px;opacity:.6;margin:-6px 0 10px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}',
      '.row{display:flex;gap:8px}',
      'button{flex:1;cursor:pointer;border:none;border-radius:8px;padding:9px 10px;font:600 13px system-ui;',
      'transition:filter .15s}',
      'button:hover{filter:brightness(1.12)}',
      '.copy{background:#39415a;color:#fff}',
      '.fill{background:#3f51b5;color:#fff}',
      '.msg{font-size:12px;opacity:.85;margin-top:8px;min-height:0}'
    ].join('');
  }

  function hide() {
    if (hideTimer) { clearTimeout(hideTimer); hideTimer = null; }
    if (host) host.style.display = 'none';
  }

  // ---- wiring --------------------------------------------------------------
  api.runtime.onMessage.addListener(function (msg, sender, sendResponse) {
    if (!msg) return;
    if (msg.type === 'otp:show') { showToast(msg.code, msg.meta); }
    if (msg.type === 'otp:autofill') {
      var res = autofill(msg.code);
      if (sendResponse) sendResponse(res);
      return true;
    }
  });

  function dismiss() {
    api.runtime.sendMessage({ type: 'otp:dismiss' }).catch(function () {});
  }

  // Auto-reshow on focus, but not once the code has been used/dismissed.
  function askLatest() {
    api.runtime.sendMessage({ type: 'otp:getLatest' }).then(function (rec) {
      if (rec && rec.code && !rec.dismissed) showToast(rec.code, rec.meta);
    }).catch(function () {});
  }

  document.addEventListener('visibilitychange', function () { if (!document.hidden) askLatest(); });
  window.addEventListener('focus', askLatest);
  askLatest();
})();
