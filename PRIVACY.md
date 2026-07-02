# Privacy Policy - Mail OTP Autofill

_Last updated: 2026-07-02_

Mail OTP Autofill helps you copy and autofill one-time verification codes from
your own email. It is a personal tool that runs entirely on your own devices and
your own mail server.

## What it accesses

- **Webmail (tab mode):** reads messages from the webmail server you configure,
  using your existing logged-in browser session, solely to detect verification
  codes.
- **Mailbox (native mode):** the optional local companion app connects to the
  IMAP server you configure, using credentials you provide, solely to detect
  verification codes. IMAP/app passwords are stored in your operating system's
  keychain and never leave your device.
- **Web pages (autofill):** when you click Copy or Autofill, it writes the code
  into the code field of the page you are on. It does not otherwise collect page
  content.

## What it stores

- The most recently detected code is held briefly (in memory and/or the
  browser's local extension storage) and cleared after use or a short expiry.
- Your settings (webmail address/server and preferences) are stored locally in
  the browser's extension storage and/or a local config file.

## What it does not do

- It sends **no data** to the project authors or any third party.
- There is **no analytics, tracking, advertising, or remote server** operated by
  this project.
- Codes and credentials never leave your device, except over the direct
  connection between the companion app and **your own** mail server.

## Permissions

- **Host/site access and scripting** are used only to read verification codes
  from your webmail and to fill a code into the site you are actively using.
- **Native messaging** is used only to receive codes from the local companion
  app on your machine.

## Contact

Questions or issues: https://github.com/m1bsvonmibenstein/mail-otp-autofill/issues
