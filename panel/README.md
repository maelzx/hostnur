# Hostnur web panel

Laravel 13, Livewire 4, Blade and Tailwind 4. One-time CLI administrator setup,
administrator login/logout, MFA enrollment and the generation-bound MFA login
challenge are available. The restricted `/panel` area requires a confirmed,
currently valid MFA session. Account recovery, recent reauthentication and real
server operations follow in the [project plan](../plan.md).

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
password event invalidates it. Account recovery and recent reauthentication for
sensitive changes are upcoming slices.

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

Stop processes with Ctrl+C. Guests hitting `/`, `/panel`, `/panel/security/mfa`
or `/two-factor-challenge` are sent to `/login`. The panel page reports the
administrator session and MFA status and links to `/panel/security/mfa`. It notes
that the agent and privileged operations are not available yet, and does not
present simulated host metrics or offer privileged actions.

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

Sessions 004–010 verified local checks with PHP 8.4.26, Composer 2.9.2, Node
24.12.0 and npm 11.6.2. The suite contains 20 administrator-bootstrap tests, 14
authentication tests, 12 MFA-enrollment tests, 13 MFA-challenge tests and two
scaffold baseline tests. Run individual suites with:

```bash
php artisan test --compact tests/Feature/AuthenticationTest.php
php artisan test --compact tests/Feature/MfaEnrollmentTest.php
php artisan test --compact tests/Feature/MfaChallengeTest.php
php artisan test --compact tests/Feature/BootstrapAdministratorTest.php
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
