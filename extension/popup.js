var api = (typeof browser !== 'undefined') ? browser : chrome;
var current = null;

function render(rec) {
  current = rec;
  var isLink = !!(rec && rec.link);
  var isCode = !!(rec && rec.code && !isLink);
  var has = isLink || isCode;
  document.getElementById('code').style.display = isCode ? 'block' : 'none';
  document.getElementById('link').style.display = isLink ? 'block' : 'none';
  document.getElementById('actions').style.display = has ? 'flex' : 'none';
  document.getElementById('none').style.display = has ? 'none' : 'block';
  document.getElementById('ttl').textContent = isLink ? 'Latest sign-in link' : 'Latest verification code';
  document.getElementById('fill').textContent = isLink ? 'Open link' : 'Autofill';
  if (isCode) document.getElementById('code').textContent = rec.code;
  if (isLink) document.getElementById('link').textContent = rec.link.host;
  if (has) {
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
  var text = current.link ? current.link.url : current.code;
  navigator.clipboard.writeText(text).then(function () {
    msg(current.link ? 'Link copied' : 'Copied ' + text);
  });
});

document.getElementById('fill').addEventListener('click', function () {
  if (!current) return;
  if (current.link) {
    api.runtime.sendMessage({ type: 'otp:open', url: current.link.url });
    window.close();
    return;
  }
  api.tabs.query({ active: true, currentWindow: true }).then(function (tabs) {
    if (!tabs[0] || tabs[0].id == null) return;
    api.tabs.sendMessage(tabs[0].id, { type: 'otp:autofill', code: current.code })
      .then(function (res) { msg(res && res.ok ? 'Filled' : (res && res.msg) || 'No field found'); window.close(); })
      .catch(function () { msg('Cannot autofill on this page'); });
  });
});

document.getElementById('opts').addEventListener('click', function () {
  api.runtime.openOptionsPage();
});
