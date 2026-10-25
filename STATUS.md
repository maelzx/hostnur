# Hostnur — current status

Last updated: 2026-10-25 (after session 054). Read this first; `plan.md` is the
detailed archive (phase designs in §5, per-session logs in §8).

## Where things stand

All planned beta features are built: panel (Laravel 13 / Livewire 4, SQLite,
DB queue, MFA), Rust root agent over a Unix socket, public/private HTTPS Git
deploys of Laravel apps as per-app Linux users with PHP-FPM + Nginx, rollback,
Let's Encrypt and uploaded certificates, MariaDB provisioning, services,
packages, firewall, schedules, encrypted (age) backups to S3, curl installer
and Debian packages. Releases up to `v0.1.5` are tagged; `main` has two
host-verified security/TLS fixes on top (`8e64f5b`, `a4b0f2b`) that need `v0.1.6`.

Session 054 ran the first full pass on a fresh droplet from the GitHub
release (`v0.1.4` install, in-place upgrade to `v0.1.5`): most rows are now
validated on a host; see `plan.md` session 054 for the evidence.

## Beta checklist (`plan.md` §7)

| # | Requirement | Status |
|---|---|---|
| 1 | Clean install, secure onboarding, MFA, session recovery | Validated from the `v0.1.4` release (054); session recovery not exercised |
| 2 | Public + private Git Laravel deploys, dedicated users, FPM pools | Public + MariaDB + migrations validated (054); private HTTPS on VM only (028); SSH deploy keys deferred |
| 3 | Builds/app commands run without root | Validated (047) |
| 4 | Uploads/secrets survive redeploy and rollback | Redeploy + rollback validated via panel (054); TLS kept on redeploy fixed (`8e64f5b`) |
| 5 | Health, logs, restarts, queue workers, scheduler | Built; partly validated (045/047) |
| 6 | Invalid Nginx change cannot replace a working config | Validated (054): refused, site and `current` untouched; health gate also refuses a 500 release |
| 7 | TLS issue/renew + uploaded certificates | Issue + uploaded install validated (054). **Defect: app certs are not refreshed after renewal** |
| 8 | Server overview, packages, services, schedules | Validated (045) |
| 9 | Jobs inspectable after browser disconnect / process interruption | Validated (054): restart mid-deploy → `execution_unknown`, processes killed, redeploy OK |
| 10 | Agent is not an arbitrary root command runner | Validated (054): full abuse suite refused; SSH-lockout rule fixed in `v0.1.5` |
| 11 | Socket/path checks, cross-app secret isolation | Validated (054) after fixing a HIGH leak (`a4b0f2b`: app users could read panel `.env`/DB and other apps' trees) — needs `v0.1.6` |
| 12 | Audit attribution, log/secret redaction | Validated (054); failed-login audit not exercised |
| 13 | SQLite queue/audit contention within limits | Validated (054): 0 lock errors under 4 writers + 10-way HTTP on 1 vCPU |
| 14 | Upgrade preserves panel data, settings, credentials | Validated `v0.1.4 → v0.1.5` (054) |
| 15 | Restore and recovery on a separate VM | Restore hardened (053); drill pending — needs a restore VM |
| 16 | Published artifacts verifiably authentic | Blocked on the signing-key decision |
| 17 | Support matrix and operator docs match tested behaviour | Pending (after the host pass) |

## Next steps

1. **Fix app certificate renewal** (row 7): certbot renews
   `/etc/letsencrypt/live/<host>` but nothing copies it into
   `/etc/hostnur/tls/<site>` or reloads nginx, so app certificates lapse ~90
   days after issue. Likely an agent hook command passed as certbot's
   `--deploy-hook` at issuance (and back-filled into existing renewal configs).
2. Tag **`v0.1.6`** with `8e64f5b` + `a4b0f2b` (+ the renewal fix) and re-run the
   isolation matrix and an upgrade from `v0.1.5` on a droplet.
3. Remaining code items, none blocking:
   - secret values held in public Livewire properties (wizard secrets,
     certificate key, DB credential fields);
   - deploy/rollback tasks still send no `timeout_seconds`;
   - agent `arguments` JSON errors are dropped silently;
   - backup: SFTP destination, S3-based restore, agent restore for
     `control_state`/`server` (manual `age -d … | tar -xz` today);
   - failed releases are never pruned from `releases/`;
   - installer prints "Create the first administrator" on upgrades;
   - agent runs as root (hardening deferred).

## Decisions only the owner can make

- Project licence (also unblocks strict Composer validation).
- Official product domain (installer/docs URLs).
- Release signing key and where the signed APT repository is hosted.
- A separate VM for the restore drill.
- Whether the installer should enable ufw (it leaves it inactive today).

## Working here

- Panel: `cd panel && php artisan test --compact`, `vendor/bin/pint`.
- Agent (Rust only builds in the Lima VM): `limactl start hostnur-rust`, then
  `limactl shell hostnur-rust -- bash -lc 'cd "<repo>/agent" && cargo clippy --all-targets -- -D warnings && cargo test'`.
  Root-only `*_vm` suites are `#[ignore]`d and need a root VM run.
- Contract: `contracts/v1/catalog.json` is shared by both sides; change it with
  both the agent dispatch and the panel caller.
- Test host: `docs/p16-test-host.md` (rebuild runbook; droplet is recreated per
  session).
