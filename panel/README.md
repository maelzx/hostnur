# Hostnur web panel

Laravel 13, Livewire 4, Blade and Tailwind 4. One-time CLI administrator setup,
administrator login/logout, MFA enrollment, the generation-bound MFA login
challenge and recent-password-confirmed security settings are available. The
restricted `/panel` area requires a confirmed, currently valid MFA session, and
sensitive security changes require a recent password confirmation. Deliberate
local account recovery and real server operations follow in the
[project plan](../plan.md).

## Local setup

Run these commands from `panel/` using existing PHP 8.3+, Composer 2.x, Node 24
and npm. They install only project-local application dependencies.

```bash
composer install --no-interaction
composer run post-root-package-install
```

The second command copies `.env.example` only when `.env` is absent. For a fresh
checkout with an empty `APP_KEY`, generate the key once:

```bash
php artisan key:generate --no-interaction
```

Keep an existing key when resuming development; it is needed to decrypt stored
application secrets. `.env` and SQLite state are ignored by Git.

Create the local database if absent, then migrate and build assets:

```bash
php -r "file_exists('database/database.sqlite') || touch('database/database.sqlite');"
php artisan migrate --no-interaction
npm ci
npm run build
```

The default SQLite path is resolved by Laravel. Leave `DB_DATABASE` unset unless
using a custom path. Sessions, cache and queues use database tables supplied by
the baseline migrations; Redis is not required.

## One-time administrator setup

After applying migrations, run this from `panel/` in your local/SSH terminal:

```bash
php artisan migrate --no-interaction
php artisan hostnur:admin:bootstrap
```

Enter the administrator name, email, password and password confirmation. Password
input is hidden, with no visible-input fallback or command-line password option.
Use at least 12 characters and at most 72 bytes (the configured bcrypt limit).
The name/email are trimmed and email is stored lowercase; passwords are hashed.

Setup succeeds only when no accounts exist and the persistent setup marker is
open. Account creation and closing that marker commit together. Validation or
creation failure leaves setup open for a corrected retry; completed setup cannot
be repeated or reopened by deleting the administrator. Missing setup state is
not silently recreated and existing accounts are not promoted or overwritten.
`--no-interaction` cannot create an account.

There is no public registration/setup route. Web login/logout is implemented
using Laravel Fortify; only the existing administrator account can authenticate,
and non-admin or unknown accounts receive the same generic failure. Successful
sign-in lands on the restricted `/panel` placeholder, which confirms the
administrator session but exposes no operations.

Multi-factor authentication is enrolled at `/panel/security/mfa`: the encrypted
TOTP secret, locally rendered QR code and one-time recovery codes are shown only
during setup, and a valid authenticator code must confirm the configuration.
Secrets are never serialized by the user model. Once confirmed, password login is
redirected to `/two-factor-challenge`, where a current TOTP code or a single-use
recovery code is required before `/panel` becomes reachable. The verified session
is bound to the account and its `mfa_version` generation, so a rotated secret or
password event invalidates it.

Sensitive changes live at `/panel/security`, which requires a password confirmed
within `HOSTNUR_RECENT_AUTH_TIMEOUT` seconds (default 300). The screen changes the
password, disables MFA and regenerates recovery codes; changing the password or
disabling MFA increments `mfa_version` and signs out existing sessions, while new
recovery codes are shown only once.

## Local administrator recovery

If the administrator password is lost, run this deliberate, interactive command
from `panel/` on the host (never over the web):

```bash
php artisan hostnur:admin:recover
```

Enter the existing administrator email, confirm the reset, then the new password
and confirmation (hidden input, at least 12 characters, at most 72 bytes). The
command refuses unknown or non-administrator emails, so it cannot create or
promote an account, and it never touches the one-time setup marker. Changing the
password increments `mfa_version`, so existing sessions and MFA proofs are
invalidated; the command also deletes the account's stored sessions. Multi-factor
authentication (secret, confirmed state and recovery codes) is preserved — it is
never silently cleared.

MFA lost-code handling, in order:

1. Sign in with a remaining one-time recovery code, then open `/panel/security`
   and use "Regenerate recovery codes" (requires recent password confirmation).
2. If no recovery codes remain, restore access with the CLI password recovery
   above; MFA stays enabled. Because the secret is preserved, the existing
   authenticator entry still works. Only if the authenticator device and all
   recovery codes are lost should the administrator sign in with the recovered
   password and re-enroll MFA from `/panel/security/mfa` (which replaces the
   secret); a dedicated CLI MFA-reset is intentionally not provided.

`--no-interaction` cannot perform recovery. Passwords, codes and secrets are
never printed or logged.

## Run

```bash
php artisan serve --host=127.0.0.1 --port=9000 --no-interaction
```

Open `http://127.0.0.1:9000`. Port 9000 is the user's selected local panel port.
For an existing `.env`, align `APP_URL` with that URL when generating absolute
links from CLI jobs; copying the example never overwrites an existing `.env`.
`/up` is the framework health endpoint. In separate
terminals, run Vite for asset development and a queue worker when jobs are added:

```bash
npm run dev
php artisan queue:work --no-interaction
```

Stop processes with Ctrl+C. Guests hitting `/`, `/panel`, `/panel/security`,
`/panel/security/mfa`, `/user/confirm-password` or `/two-factor-challenge` are
sent to `/login`. The panel page reports the MFA-verified administrator session
and links to `/panel/security`. It notes that the agent and privileged operations
are not available yet, and does not present simulated host metrics or offer
privileged actions.

## Checks

```bash
composer validate
composer check-platform-reqs
composer audit
composer check:syntax
composer check:style
php artisan test --compact
vendor/bin/pint --dirty --format agent
npm audit
npm run build
```

Sessions 004–015 verified local checks with PHP 8.4.26, Composer 2.9.2, Node
24.12.0 and npm 11.6.2. The suite contains 20 administrator-bootstrap tests, 14
authentication tests, 12 MFA-enrollment tests, 13 MFA-challenge tests, 17
security-settings tests, 8 administrator-recovery tests, 16 agent-contract tests,
8 agent-availability tests, 6 fake-agent tests and two scaffold baseline tests.
Run individual suites with:

```bash
php artisan test --compact tests/Feature/AuthenticationTest.php
php artisan test --compact tests/Feature/MfaEnrollmentTest.php
php artisan test --compact tests/Feature/MfaChallengeTest.php
php artisan test --compact tests/Feature/SecuritySettingsTest.php
php artisan test --compact tests/Feature/BootstrapAdministratorTest.php
php artisan test --compact tests/Feature/RecoverAdministratorTest.php
php artisan test --compact tests/Unit/AgentContractTest.php
php artisan test --compact tests/Unit/AgentAvailabilityTest.php
php artisan test --compact tests/Unit/FakeAgentTest.php
```

Authentication tests use isolated in-memory SQLite and cover guest redirects,
admin-only login, generic failures, immediate role removal, session ID rotation,
logout/session invalidation, named rate limiting with expiry and a
production-like CSRF rejection check. They never create your real administrator.

MFA-enrollment tests use isolated in-memory SQLite and cover the administrator
gate, encrypted secret/recovery-code storage, hidden serialization, wrong and
malformed codes, leading-zero preservation, confirmation state and the
show-once behavior. They use Fortify's encrypted storage, so the application key
(`APP_KEY`) must be present; they never touch your real administrator.

MFA-challenge tests cover password-only denial, the challenge screen, valid and
invalid TOTP codes, single-use recovery codes, the generation-bound session
proof, stale-proof rejection, unconfirmed enrollment redirects and Livewire
persistent middleware. They also never create your real administrator.

Security-settings tests cover the recent-auth window and MFA-generation binding,
password confirmation success/failure, mutation blocking without recent auth,
password change and MFA disable revocation, recovery-code regeneration and the
once-only rendering, plus recent-auth protection of Fortify's own MFA endpoints.

Administrator-recovery tests cover success with preserved MFA/setup state and
invalidated sessions, unknown/non-admin targets, non-interactive refusal,
cancellation, short/mismatched passwords, write-failure rollback and hidden,
fallback-free password prompts. They never touch your real administrator.

Agent-contract tests cover the operation catalog, valid request fixtures parsing
into typed DTOs, request envelope round-tripping, invalid/excess/unknown/oversized
input rejection with the expected error codes, and typed receipt/snapshot/
capability parsing. They are pure contract tests with no network or host calls.

The versioned agent contract lives outside the app at `../contracts/v1/`
(JSON Schemas, operation catalog and valid/invalid fixtures); `HOSTNUR_CONTRACTS_PATH`
overrides its location and `HOSTNUR_AGENT_MODE` selects the agent mode. The default
`unavailable` mode binds `UnavailableAgentClient`, which reports no capabilities
and no observations and fails every mutation closed. `real` also reports
unavailable until phase 11. In local/testing, `fake` binds `FakeAgentClient`, which
returns fixture-driven capabilities and server snapshots labelled `mode=fake` with
a controlled timestamp (no network, host or shell calls); it is rejected in
production and does not implement durable execution yet (next slices).

Agent-availability and fake-agent tests cover the default/`real` unavailable
binding, fail-closed mutations, the production fake guard, unknown modes, and the
fake capabilities/snapshot fixture shape, determinism, timestamp and
fresh/stale/offline distinction. They make no network or host calls.

Bootstrap tests use isolated in-memory SQLite, including interleaved setup writes,
rollback and secret-output checks; they do not create your real administrator.
Composer resolves against platform PHP 8.3.0. Actual PHP 8.3/8.4 workflow outcomes
are recorded in `../plan.md`.

`composer validate` currently reports a missing project-license warning because
Hostnur's license is still an open product decision. Application Composer/npm
audits passed. Earlier Composer-tool diagnostic findings are recorded separately
in [development-toolchain.md](../docs/development-toolchain.md).

Laravel Boost is included as a development dependency as required by the original
scaffold's repository guidance. It is not a globally installed tool or production
runtime requirement. Hostnur-specific instructions are in `AGENTS.md`.

## Continuous integration

`.github/workflows/web-panel.yml` runs on panel/workflow/Node-version changes
pushed to `main`, matching pull requests, and manual dispatch. Each PHP 8.3/8.4
job uses Ubuntu 24.04, Node 24 and Composer 2.10.3 in the CI runner.

The jobs install the committed lockfiles, check actual platform requirements,
audit dependencies, lint PHP syntax, enforce Pint formatting, build assets,
initialize an isolated SQLite database, run migrations, compile templates,
check empty queue startup and execute panel tests. No application secrets or
Rust tooling are required. `composer validate` keeps the known license warning
visible; invalid metadata or a stale lock still fails the job.

`composer check:style` runs Pint and fails if tracked PHP files differ from Git,
so review/commit intentional PHP changes before expecting this gate to pass.
The syntax command checks tracked PHP files; stage new files before checking.
These gates add no new local tools. Actual remote results are recorded in
`../plan.md` with the corresponding GitHub Actions run.
