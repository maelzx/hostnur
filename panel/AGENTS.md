# Hostnur panel guidance

- Read the current handoff in `../plan.md` before beginning a slice. Work is
  web-first; Rust development and privileged host operations are deferred.
- Use existing runtimes/tools. Project-local dependencies are allowed; never
  install or upgrade system packages, global tools, PHP/Node or Rust here.
- Laravel Boost is already installed as a development dependency. Its generated
  framework guidance is in `CLAUDE.md`; follow it with these Hostnur constraints.
- Confirm installed APIs with `composer show --direct` and the JS lockfile.
- Keep Laravel conventions and use Artisan generators with `--no-interaction`.
- PHP minimum is 8.3, even when developing on 8.4. Use explicit parameter and
  return types. Run `vendor/bin/pint --dirty --format agent` after PHP changes.
- Use Blade/Livewire and Tailwind. Livewire bundles Alpine; do not load it twice.
  HTMX belongs only in a separately owned flow when a concrete need arises.
- The root page is a non-interactive development foundation, not an authenticated
  dashboard. Build administrator bootstrap/login/MFA in its planned phase.
- Keep `.env`, keys, SQLite files, logs, vendor/node_modules and generated builds
  untracked. Use committed Composer/npm lockfiles for reproducible installation.
- Real privileged operations require the later agent; do not implement root
  commands in PHP. Restrict future fake adapters to clearly labeled local/tests.
- Run the relevant checks documented in `README.md`. Record exact results and
  any blocked checks in `../plan.md`, then commit/push at the session checkpoint.
