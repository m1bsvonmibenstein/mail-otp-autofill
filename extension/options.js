var api = (typeof browser !== 'undefined') ? browser : chrome;

var DEFAULTS = {
  source: 'tab',
  origin: '',
  login: '',
  folder: '0/folderINBOX',
  pollIntervalMs: 20000,
  codeTtlMs: 300000,
  restoreUnread: true,
  notify: true,
  enabled: true
};

// The origin field must be scheme+host only: it feeds both the content-script
// match pattern AND the poller's API base (origin + '/SOGo/so/...'). Users paste
// the full webmail URL (often with a '/SOGo' or lowercase '/sogo' path), which
// breaks the case-sensitive match pattern, so collapse anything to its origin.
function normOrigin(s) {
  s = (s || '').trim();
  if (!s) return '';
  if (!/^[a-z][a-z0-9+.-]*:\/\//i.test(s)) s = 'https://' + s;
  try { return new URL(s).origin; } catch (e) { return s.replace(/([^/])\/.*$/, '$1').replace(/\/+$/, ''); }
}

function originPattern(origin) {
  var o = normOrigin(origin);
  return o ? o + '/*' : '';
}

function toggleTabFields() {
  var isTab = document.querySelector('input[name=source]:checked').value === 'tab';
  document.getElementById('tab-fields').style.display = isTab ? 'block' : 'none';
  document.getElementById('native-note').style.display = isTab ? 'none' : 'block';
}

function load() {
  api.storage.local.get('otpConfig').then(function (o) {
    var c = Object.assign({}, DEFAULTS, (o && o.otpConfig) || {});
    document.querySelector('input[name=source][value=' + (c.source === 'native' ? 'native' : 'tab') + ']').checked = true;
    document.getElementById('origin').value = c.origin;
    document.getElementById('login').value = c.login;
    document.getElementById('folder').value = c.folder;
    document.getElementById('pollSec').value = Math.round(c.pollIntervalMs / 1000);
    document.getElementById('ttlSec').value = Math.round(c.codeTtlMs / 1000);
    document.getElementById('restoreUnread').checked = !!c.restoreUnread;
    document.getElementById('notify').checked = !!c.notify;
    document.getElementById('enabled').checked = !!c.enabled;
    toggleTabFields();
  });
}

function flashSaved() {
  var s = document.getElementById('saved');
  s.style.opacity = '1';
  setTimeout(function () { s.style.opacity = '0'; }, 1500);
}
function flashMsg(t, ok) {
  var el = document.getElementById('msg');
  el.textContent = t || '';
  el.style.color = ok ? '#16a34a' : '#b91c1c';
}

// Direct native-messaging probe from the options page - reports hello/pong or the
// exact lastError (e.g. "host not found"), so native setup can be diagnosed.
function testNative() {
  flashMsg('Testing native app…', true);
  var port, done = false;
  try { port = api.runtime.connectNative('com.mibs.otp_relay'); }
  catch (e) { flashMsg('connectNative threw: ' + e.message); return; }
  var timer = setTimeout(function () {
    if (done) return; done = true;
    try { port.disconnect(); } catch (e) {}
    flashMsg('No reply from native app within 3s (is it hanging?).');
  }, 3000);
  port.onMessage.addListener(function (m) {
    if (done) return; done = true; clearTimeout(timer);
    try { port.disconnect(); } catch (e) {}
    flashMsg('Native app connected ✓  reply: ' + JSON.stringify(m), true);
  });
  port.onDisconnect.addListener(function () {
    if (done) return; done = true; clearTimeout(timer);
    var err = (api.runtime.lastError && api.runtime.lastError.message) || 'disconnected with no message';
    flashMsg('Native app error: ' + err);
  });
}

async function save() {
  flashMsg('');
  var source = document.querySelector('input[name=source]:checked').value;
  var cfg = {
    source: source,
    origin: normOrigin(document.getElementById('origin').value),
    login: document.getElementById('login').value.trim(),
    folder: document.getElementById('folder').value.trim() || DEFAULTS.folder,
    pollIntervalMs: Math.max(10, parseInt(document.getElementById('pollSec').value, 10) || 20) * 1000,
    codeTtlMs: Math.max(30, parseInt(document.getElementById('ttlSec').value, 10) || 300) * 1000,
    restoreUnread: document.getElementById('restoreUnread').checked,
    notify: document.getElementById('notify').checked,
    enabled: document.getElementById('enabled').checked
  };

  // In tab mode we need host permission for the webmail origin. permissions.request
  // must run inside this click gesture, so do it before persisting.
  if (source === 'tab') {
    var pat = originPattern(cfg.origin);
    if (!pat) { flashMsg('Enter your webmail origin (e.g. https://mail.example.com).'); return; }
    var ok = false;
    try { ok = await api.permissions.request({ origins: [pat] }); } catch (e) { ok = false; }
    if (!ok) { flashMsg('Permission for ' + cfg.origin + ' was declined - polling cannot run.'); return; }
  }

  await api.storage.local.set({ otpConfig: cfg });
  try { await api.runtime.sendMessage({ type: 'otp:reconfigure' }); } catch (e) {}
  flashSaved();
}

Array.prototype.forEach.call(document.querySelectorAll('input[name=source]'), function (el) {
  el.addEventListener('change', toggleTabFields);
});
function refreshDbg() {
  api.storage.local.get('otpDbg').then(function (o) {
    var a = (o && o.otpDbg) || [];
    document.getElementById('dbg').textContent = a.length ? a.join('\n') : '(no events yet)';
  });
}
function clearDbg() { api.storage.local.set({ otpDbg: [] }).then(refreshDbg); }

document.getElementById('save').addEventListener('click', save);
document.getElementById('test-native').addEventListener('click', testNative);
document.getElementById('refresh-dbg').addEventListener('click', refreshDbg);
document.getElementById('clear-dbg').addEventListener('click', clearDbg);
load();
refreshDbg();
