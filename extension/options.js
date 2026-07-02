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

function originPattern(origin) {
  var o = (origin || '').trim().replace(/\/+$/, '');
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
function flashMsg(t) { document.getElementById('msg').textContent = t || ''; }

async function save() {
  flashMsg('');
  var source = document.querySelector('input[name=source]:checked').value;
  var cfg = {
    source: source,
    origin: document.getElementById('origin').value.trim().replace(/\/+$/, ''),
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
    if (!ok) { flashMsg('Permission for ' + cfg.origin + ' was declined — polling cannot run.'); return; }
  }

  await api.storage.local.set({ otpConfig: cfg });
  try { await api.runtime.sendMessage({ type: 'otp:reconfigure' }); } catch (e) {}
  flashSaved();
}

Array.prototype.forEach.call(document.querySelectorAll('input[name=source]'), function (el) {
  el.addEventListener('change', toggleTabFields);
});
document.getElementById('save').addEventListener('click', save);
load();
