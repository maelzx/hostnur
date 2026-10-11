# Hostnur web panel

Laravel 13, Livewire 4, Blade and Tailwind 4. The current root page is a
non-interactive development foundation. Administrator onboarding, authentication
and real server operations are scheduled in the [project plan](../plan.md).

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

## Run

```bash
php artisan serve --host=127.0.0.1 --port=8000 --no-interaction
```

Open `http://127.0.0.1:8000`. `/up` is the framework health endpoint. In separate
terminals, run Vite for asset development and a queue worker when jobs are added:

```bash
npm run dev
php artisan queue:work --no-interaction
```

Stop processes with Ctrl+C. The root page reports the agent as not connected;
it does not present simulated host metrics or offer privileged actions.

## Checks

```bash
composer validate
composer check-platform-reqs
composer audit
php artisan test --compact
vendor/bin/pint --dirty --format agent
npm audit
npm run build
```

Session 004 verified these with PHP 8.4.26, Composer 2.9.2, Node 24.12.0 and npm
11.6.2. The scaffold includes two baseline tests; broader behavior tests will
accompany their respective features. PHP 8.3 execution and CI remain next-slice
work. Composer resolves against platform PHP 8.3.0; that is not an actual PHP 8.3
runtime test.

`composer validate` currently reports a missing project-license warning because
Hostnur's license is still an open product decision. Application Composer/npm
audits passed. Earlier Composer-tool diagnostic findings are recorded separately
in [development-toolchain.md](../docs/development-toolchain.md).

Laravel Boost is included as a development dependency as required by the original
scaffold's repository guidance. It is not a globally installed tool or production
runtime requirement. Hostnur-specific instructions are in `AGENTS.md`.
