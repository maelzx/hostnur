# Authentication and recovery

Hostnur uses Laravel's session guard with Laravel Fortify for the login/logout,
password-confirmation and TOTP pipeline. There is no public registration or web
setup route.

## Account creation (one-time CLI)

```bash
php artisan hostnur:admin:bootstrap
```

Interactive only: name, email, and hidden password/confirmation (minimum 12
characters, at most 72 bytes, no null bytes). Account creation and the durable
`panel_setup` marker commit together in one transaction. Setup stays open on
failure; completed setup cannot be reopened by deleting the administrator, and
existing ordinary accounts are never promoted. No default credentials are seeded.

## Login

- Only an existing `is_admin=true` account authenticates; unknown email, ordinary
  account and wrong password all return the same generic failure.
- Named rate limiting: 5 attempts/minute per normalized email + IP and 30/minute per
  IP.
- The session ID rotates after password authentication. No remember-me.
- Every authorized request reaches the restricted panel through
  `auth` → `administrator` → `mfa` → (`recent-auth` where required) gates.

## MFA (TOTP)

- Enroll at `/panel/security/mfa`: a Fortify-generated secret is stored encrypted
  (text column, encrypted by Fortify's encrypter), rendered locally as a QR code,
  and shown with one-time recovery codes only until confirmed.
- A confirmed administrator is challenged at `/two-factor-challenge`; a valid TOTP
  code or a single-use recovery code completes login. TOTP replay is rejected by
  Fortify's cache-backed `verifyKeyNewer`.
- The verified session is bound to the user id and the persisted `mfa_version`
  generation. Rotating the password or MFA increments `mfa_version`, so older
  sessions and proofs are rejected by `EnsureMfaVerified`.

## Recent authentication

Sensitive changes (password change, MFA disable/regeneration, deployment/Restart/
rollback, server-administration requests) require a password confirmation within
`HOSTNUR_RECENT_AUTH_TIMEOUT` seconds (default 300), stored server-side as
`auth.password_confirmed_at` plus a generation-bound version. The confirmation is
never a client-provided boolean. `auth.password_timeout` mirrors the same window.

## Session revocation

- Changing the password or disabling MFA bumps `mfa_version`, clears the
  confirmation and signs out the current session; other sessions become stale.
- Fortify's own `/user/two-factor-*` endpoints require Laravel's `password.confirm`
  middleware, so a verified administrator cannot rotate MFA without re-entering the
  password.

## Recovery

No public recovery endpoint and no email reset (mail is not configured). Use the
deliberate local CLI:

```bash
php artisan hostnur:admin:recover
```

It targets an existing administrator only, requires interactive confirmation and a
new hidden password, preserves MFA and the setup marker, bumps `mfa_version` and
deletes the account's stored sessions. MFA lost-code handling: use a remaining
recovery code, then `/panel/security` regeneration; otherwise recover the password
by CLI (MFA stays enabled), and re-enroll only if the device and all recovery codes
are lost.

## Deployment prerequisites

Production requires HTTPS (secure/HttpOnly/SameSite cookies), accurate server time
for TOTP, and the application key kept outside the repository. Loss of `APP_KEY`
prevents decryption of stored secrets/credentials and fails safely rather than
returning empty values.
