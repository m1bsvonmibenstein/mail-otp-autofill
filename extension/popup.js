var api = (typeof browser !== 'undefined') ? browser : chrome;
var current = null;

function render(rec) {
  current = rec;
  var has = rec && rec.code;
  document.getElementById('code').style.display = has ? 'block' : 'none';
  document.getElementById('actions').style.display = has ? 'flex' : 'none';
  document.getElementById('none').style.display = has ? 'none' : 'block';
  if (has) {
    document.getElementById('code').textContent = rec.code;
    var sub = document.getElementById('sub');
    var from = rec.meta && rec.meta.from ? (rec.meta.from.name || rec.meta.from.email || '') : '';
    var line = [from, rec.meta && rec.meta.subject].filter(Boolean).join(' - ');
    if (line) { sub.textContent = line; sub.style.display = 'block'; }
    else sub.style.display = 'none';
  }
}

function msg(t) { document.getElementById('msg').textContent = t; }

api.runtime.sendMessage({ type: 'otp:getLatest' }).then(render).catch(function () { render(null); });

document.getElementById('copy').addEventListener('click', function () {
  if (!current) return;
  navigator.clipboard.writeText(current.code).then(function () { msg('Copied ' + current.code); });
});

document.getElementById('fill').addEventListener('click', function () {
  if (!current) return;
  api.tabs.query({ active: true, currentWindow: true }).then(function (tabs) {
    if (!tabs[0] || tabs[0].id == null) return;
    api.tabs.sendMessage(tabs[0].id, { type: 'otp:autofill', code: current.code })
      .then(function (res) { msg(res && res.ok ? 'Filled ✓' : (res && res.msg) || 'No field found'); window.close(); })
      .catch(function () { msg('Cannot autofill on this page'); });
  });
});

document.getElementById('opts').addEventListener('click', function () {
  api.runtime.openOptionsPage();
});
