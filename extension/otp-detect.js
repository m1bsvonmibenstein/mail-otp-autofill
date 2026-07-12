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
  // Iteration copies (global) + phone-rejection patterns, kept in sync with otp.rs.
  var KW_G = new RegExp(KEYWORDS.source, 'ig');
  var TOKEN_G = new RegExp(TOKEN.source, 'g');
  var PHONE_WORDS = /(?:call|phone|tel|dial|contact|text\s+us|\bsms\b|fax|mobile|hotline|help\s?line|toll[\s.\-]?free|whats\s?app)\b/i;
  var PHONE_SHAPE = /(?:\+\d{1,3}[\s.\-]*)?\(?\d{3}\)?[\s.\-]+\d{3}[\s.\-]+\d{4}|1[\s.\-]?\d{3}[\s.\-]?\d{3}[\s.\-]?\d{4}/g;

  function normalize(s) { return (s || '').replace(/[\s-]/g, ''); }

  // URLs / emails are removed before code scanning (embedded UUIDs, tracking ids
  // and path keywords like "/verification/" are a major false-positive source).
  // Links are still handled separately by findLink. Kept in sync with otp.rs.
  var URL_EMAIL = /https?:\/\/\S+|[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}/ig;
  function stripUrls(text) { return (text || '').replace(URL_EMAIL, ' '); }

  // True if the token at [start,end) in ctx is part of a phone number.
  function isPhone(ctx, start, end) {
    var before = ctx.slice(Math.max(0, start - 25), start);
    if (PHONE_WORDS.test(before)) return true;
    PHONE_SHAPE.lastIndex = 0;
    var pm;
    while ((pm = PHONE_SHAPE.exec(ctx)) !== null) {
      var ps = pm.index, pe = pm.index + pm[0].length;
      if (ps < end && start < pe) return true;
      if (pm.index === PHONE_SHAPE.lastIndex) PHONE_SHAPE.lastIndex++;
    }
    return false;
  }

  // First token in s that has a digit (so all-caps words can't match the alnum
  // branch) and is not part of a phone number. Iterates so a phone can't shadow
  // a later code.
  function firstValid(s) {
    TOKEN_G.lastIndex = 0;
    var m;
    while ((m = TOKEN_G.exec(s)) !== null) {
      var t = m[1], start = m.index, end = m.index + m[0].length;
      if (/[0-9]/.test(t) && !isPhone(s, start, end)) return normalize(t);
      if (m.index === TOKEN_G.lastIndex) TOKEN_G.lastIndex++;
    }
    return null;
  }

  // Magic sign-in links: an https URL sitting near a login/verify keyword.
  // Proximity keeps footer/unsubscribe/social links from being surfaced, and a
  // small denylist drops the obvious non-auth links that slip through.
  var LINK_KEYWORDS = /(magic\s?link|passwordless|sign[\s-]?in|log[\s-]?in|finish\s+(?:signing|sign)|continue\s+to\s+(?:sign|log)|one[\s-]?time\s+(?:sign|log|link))/i;
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
    text = stripUrls(text);
    var lines = text.split(/\n+/), i, j, c;
    for (i = 0; i < lines.length; i++) {
      if (!KEYWORDS.test(lines[i])) continue;
      c = firstValid(lines[i]);
      if (c) return c;
      for (j = 1; j <= 2 && i + j < lines.length; j++) {
        c = firstValid(lines[i + j]);
        if (c) return c;
      }
    }
    // Fallback: a token within ~60 chars after any keyword occurrence.
    KW_G.lastIndex = 0;
    var km;
    while ((km = KW_G.exec(text)) !== null) {
      var end = km.index + km[0].length;
      c = firstValid(text.slice(end, end + 60));
      if (c) return c;
      if (km.index === KW_G.lastIndex) KW_G.lastIndex++;
    }
    return null;
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
