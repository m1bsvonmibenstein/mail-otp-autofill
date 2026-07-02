// Shared OTP detection. Loaded (via <script> concat order or duplicated intent)
// but here it is a standalone module attached to a global so poller/popup reuse it.
// Content scripts in the same document/world share globals; the poller loads this
// logic inline instead to stay dependency-free. This file documents the canonical
// regex so all copies stay in sync.
(function (root) {
  'use strict';
  // Keyword must sit near the token, or we'd grab order numbers / dates / phones.
  var KEYWORDS = /(one[\s-]?time|verification|verify|security|passcode|pass\s?code|access\s?code|confirmation|auth(?:entication)?|login|sign[\s-]?in|OTP|2FA|MFA|\bcode\b)/i;
  var TOKEN = /\b([0-9]{3}[\s-]?[0-9]{3}|[0-9]{4,8}|[A-Z0-9]{6,8})\b/;

  function normalize(s) { return (s || '').replace(/[\s-]/g, ''); }

  function findCode(text) {
    if (!text) return null;
    var lines = text.split(/\n+/), best = null, i, j;
    for (i = 0; i < lines.length; i++) {
      if (!KEYWORDS.test(lines[i])) continue;
      var m = lines[i].match(TOKEN);
      if (m) { best = normalize(m[1]); break; }
      for (j = 1; j <= 2 && i + j < lines.length; j++) {
        var m2 = lines[i + j].match(TOKEN);
        if (m2) { best = normalize(m2[1]); break; }
      }
      if (best) break;
    }
    if (!best) {
      var win = text.match(new RegExp(KEYWORDS.source + '[^0-9A-Za-z]{0,40}' + TOKEN.source, 'i'));
      if (win) { var mm = win[0].match(TOKEN); if (mm) best = normalize(mm[1]); }
    }
    return best;
  }

  root.OTP = { findCode: findCode, KEYWORDS: KEYWORDS, TOKEN: TOKEN };
})(typeof self !== 'undefined' ? self : this);
