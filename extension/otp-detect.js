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

  // Magic sign-in links: an https URL sitting near a login/verify keyword.
  // Proximity keeps footer/unsubscribe/social links from being surfaced, and a
  // small denylist drops the obvious non-auth links that slip through.
  var LINK_KEYWORDS = /(magic\s?link|sign[\s-]?in|log[\s-]?in|verify|confirm(?:ation)?|activate|authenticat|one[\s-]?time|access\s+your|complete\s+your|finish\s+(?:signing|sign)|continue\s+to\s+(?:sign|log))/i;
  var LINK_DENY = /(unsubscribe|unsub\b|opt[\s-]?out|list-manage|list-unsubscribe|\/preferences|email[\s-]?settings|manage[\s-]?(?:your[\s-]?)?preferences)/i;
  var URL_RE = /https:\/\/[^\s<>"'`\)\]]+/ig;

  function hostOf(url) {
    try { return new URL(url).host; } catch (e) { return null; }
  }

  function findLink(text) {
    if (!text) return null;
    URL_RE.lastIndex = 0;
    var m;
    while ((m = URL_RE.exec(text)) !== null) {
      var url = m[0].replace(/[.,;:!?]+$/, '');
      if (LINK_DENY.test(url)) continue;
      var host = hostOf(url);
      if (!host) continue;
      var from = Math.max(0, m.index - 160);
      var ctx = text.slice(from, m.index + url.length + 40);
      if (LINK_KEYWORDS.test(ctx) || LINK_KEYWORDS.test(url)) {
        return { url: url, host: host };
      }
    }
    return null;
  }

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

  // HTML fallback: many magic-link mails are HTML-only, so SOGo's plaintext
  // conversion drops the <a href>. Pull the anchor whose visible text or href
  // carries a login/verify keyword (same deny/https rules as findLink).
  function stripTags(s) {
    return (s || '').replace(/<[^>]*>/g, ' ').replace(/&nbsp;/gi, ' ').replace(/\s+/g, ' ').trim();
  }
  function decodeEntities(s) {
    return (s || '')
      .replace(/&#x([0-9a-f]+);/gi, function (_, h) { try { return String.fromCharCode(parseInt(h, 16)); } catch (e) { return _; } })
      .replace(/&#(\d+);/g, function (_, n) { try { return String.fromCharCode(parseInt(n, 10)); } catch (e) { return _; } })
      .replace(/&amp;/gi, '&');
  }
  var ANCHOR_RE = /<a\b[^>]*?href\s*=\s*(["'])(https:\/\/[^"']+)\1[^>]*>([\s\S]*?)<\/a>/ig;

  function findLinkInHtml(html) {
    if (!html) return null;
    ANCHOR_RE.lastIndex = 0;
    var m;
    while ((m = ANCHOR_RE.exec(html)) !== null) {
      var url = decodeEntities(m[2]);
      if (LINK_DENY.test(url)) continue;
      var host = hostOf(url);
      if (!host) continue;
      var text = stripTags(m[3]);
      if (LINK_KEYWORDS.test(text) || LINK_KEYWORDS.test(url)) {
        return { url: url, host: host };
      }
    }
    return null;
  }

  root.OTP = { findCode: findCode, findLink: findLink, findLinkInHtml: findLinkInHtml, KEYWORDS: KEYWORDS, TOKEN: TOKEN };
})(typeof self !== 'undefined' ? self : this);
