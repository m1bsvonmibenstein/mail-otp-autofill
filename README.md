# Mail OTP Autofill

Pulls one-time verification codes out of your webmail the moment they arrive and
gives you one-click **copy** and **autofill** on whatever site you're on, a
top-right toast, a browser notification, and a toolbar popup.

Built for self-hosted **mailcow / SOGo** first, with a provider-adapter design so
other mail sources slot in.

## Two code sources (pick in the extension's Settings)

| Mode | How it gets codes | Trade-off |
|------|-------------------|-----------|
| **Webmail tab** | A content script in your open webmail tab reads new mail via the SOGo API (same-origin, uses your existing login). | Zero install, **no stored credentials**. A webmail tab must stay open; ~60s latency when it's backgrounded. |
| **Native app** | An always-on local daemon (`native-app/`) watches your mailbox over IMAP IDLE and pushes codes to a thin native-messaging bridge the browser spawns. | **No tab needed**, real-time, works even with the browser closed (desktop notification + optional auto-copy). Requires installing the companion app; it holds IMAP app passwords in the OS keychain. |

The extension is the same in both modes - only the *source* of the code changes.

## Security model

- **Codes are ephemeral:** held in memory only, never written to disk or logged, cleared ~30s after you copy/autofill (or a short max-TTL).
- **Tab mode stores no credentials at all** - it only reads what your logged-in browser session can already see.
- **Native mode** keeps IMAP app passwords in the OS keychain (Windows Credential Manager / macOS Keychain / Linux Secret Service), and talks to the extension over **native messaging** (only the pinned extension ID can reach it - not websites, unlike a localhost port).

## Install

### Extension
- **Chrome / Edge / Brave / Opera:** `chrome://extensions` → enable Developer mode → **Load unpacked** → select `extension/`.
- **Firefox:** `about:debugging` → This Firefox → **Load Temporary Add-on** → select `extension/manifest.json`.

Then open the extension's **Settings**, choose a code source, and (tab mode) enter your webmail origin and Save - you'll be prompted to grant access to that server.

### Native app (optional, for "no tab" mode)

**Easiest (Windows):** run the one-click installer `MailOtpAutofill-Setup.exe` (from
Releases, or build it with `installer/mail-otp-autofill.iss` via Inno Setup). It
installs per-user, registers the native host, autostarts the background daemon, and
adds a Start Menu shortcut - then opens the GUI so you can add accounts.

**From source:** requires the [Rust toolchain](https://rustup.rs).

```powershell
cd native-app
powershell -ExecutionPolicy Bypass -File .\install-windows.ps1
```

This builds the host + GUI, registers the host for Chrome, Edge, and Firefox
(current user), and adds a Start Menu shortcut ("Mail OTP Autofill").

Add mailboxes with the **GUI** (Start Menu, or run `otp-relay-gui`), or the CLI
(the app password goes into the OS keychain, never on disk):

```powershell
.\target\release\otp-relay.exe add --label mailcow --host mail.example.com --user you@example.com
.\target\release\otp-relay.exe test    # verify the connection
.\target\release\otp-relay.exe list
```

#### Providers

Any IMAP mailbox works. The GUI has one-click presets for the common ones:

| Provider | IMAP host | Notes |
|----------|-----------|-------|
| mailcow / self-hosted | your mail host | mailbox password or a mailcow app password |
| Gmail | `imap.gmail.com` | requires 2-step verification + a Google **app password** |
| Outlook / Microsoft 365 | `outlook.office365.com` | requires an app password |
| iCloud | `imap.mail.me.com` | requires an app password |

Gmail/Outlook/iCloud reject your normal password over IMAP - generate an app
password in the account's security settings and use that.

Finally, set the extension's code source to **Native app**. New mail is watched in
real time over IMAP IDLE and codes are pushed to the extension; messages are read
with `BODY.PEEK`, so they are never marked read.

## Roadmap

- [x] **Native app: always-on daemon + bridge** - IMAP IDLE watcher (mailcow + any IMAP), keychain-backed, autostarts at login, GUI account manager.
- [x] **Multi-account** - watch several mailboxes; codes are tagged by account.
- [x] **Native app: auto copy to clipboard** - optionally copy the code the instant it arrives (toggle in the GUI).
- [x] **Native app: desktop notifications** - OS-level notification from the daemon, independent of the browser (Windows may need an AppUserModelID to display).
- [x] **Gmail / Outlook / iCloud** - via IMAP + app password in native mode (GUI presets).
- [ ] **Gmail tab adapter** - content-script source for the open Gmail tab (only needed for tab mode, no OAuth).
- [ ] **Signed builds** - code-sign the native binary (Windows) and notarize (macOS) to drop install warnings.
- [ ] **Store listings** - Chrome Web Store + Firefox AMO + Edge/Opera.

## Layout

```
extension/    browser extension (MV3, Chrome/Edge/Firefox)
native-app/   Rust native-messaging host (IMAP IDLE watcher)
```

## License

MIT, see [LICENSE](LICENSE).
