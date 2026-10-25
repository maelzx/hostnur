# Hostnur — current status

Last updated: 2026-10-25 (after session 053). Read this first; `plan.md` is the
detailed archive (phase designs in §5, per-session logs in §8).

## Where things stand

All planned beta features are built: panel (Laravel 13 / Livewire 4, SQLite,
DB queue, MFA), Rust root agent over a Unix socket, public/private HTTPS Git
deploys of Laravel apps as per-app Linux users with PHP-FPM + Nginx, rollback,
Let's Encrypt and uploaded certificates, MariaDB provisioning, services,
packages, firewall, schedules, encrypted (age) backups to S3, curl installer
and Debian packages. Releases up to `v0.1.3` are tagged.

**The gap is evidence, not features**: no item of the beta checklist below has
been signed off on a fresh host since the latest changes.

Recent architecture changes (sessions 050–053) that the next host run must
exercise: mutations run asynchronously on a serialized agent worker with
enforced timeouts; rollback names the agent's release ids; uploaded
certificates install via staged material; restore swaps a staged tree in
instead of writing into `shared/`; `control_state`/`server` backup scopes and
local retention.

## Beta checklist (`plan.md` §7)

| # | Requirement | Status |
|---|---|---|
| 1 | Clean install, secure onboarding, MFA, session recovery | Built; installer used on droplets (041–047); re-validate from a tagged release |
| 2 | Public + private Git Laravel deploys, dedicated users, FPM pools | Public validated on host (047); private HTTPS validated on VM (028); SSH deploy keys deferred |
| 3 | Builds/app commands run without root | Validated (047) |
| 4 | Uploads/secrets survive redeploy and rollback | Built; rollback newly wired (051) — validate with `hostnur:smoke --rollback` |
| 5 | Health, logs, restarts, queue workers, scheduler | Built; partly validated (045/047) |
| 6 | Invalid Nginx change cannot replace a working config | Built (transactional apply, P14); not exercised on host |
| 7 | TLS issue/renew + uploaded certificates | Issue validated (047); renewal hook (049) and uploaded install (051) unvalidated |
| 8 | Server overview, packages, services, schedules | Validated (045) |
| 9 | Jobs inspectable after browser disconnect / process interruption | Async worker + restart sweep (050); unvalidated on host |
| 10 | Agent is not an arbitrary root command runner | Error framing / allowlists validated (047); full abuse suite pending |
| 11 | Socket/path checks, cross-app secret isolation | Pending (two-app test) |
| 12 | Audit attribution, log/secret redaction | Built; needs a review pass with evidence |
| 13 | SQLite queue/audit contention within limits | Not tested |
| 14 | Upgrade preserves panel data, settings, credentials | Not tested |
| 15 | Restore and recovery on a separate VM | Restore hardened (053); drill pending — needs a restore VM |
| 16 | Published artifacts verifiably authentic | Blocked on the signing-key decision |
| 17 | Support matrix and operator docs match tested behaviour | Pending (after the host pass) |

## Next steps

1. **Droplet validation pass** (needs a fresh Ubuntu 24.04 amd64 droplet + DNS
   from the owner). Follow `docs/p16-test-host.md`; drive deploys with
   `php artisan hostnur:smoke` (§7b) and record evidence per checklist row.
2. Remaining code items, none blocking the pass:
   - secret values held in public Livewire properties (wizard secrets,
     certificate key, DB credential fields);
   - deploy/rollback tasks still send no `timeout_seconds`;
   - agent `arguments` JSON errors are dropped silently;
   - backup: SFTP destination, S3-based restore, agent restore for
     `control_state`/`server` (manual `age -d … | tar -xz` today);
   - agent runs as root (hardening deferred).

## Decisions only the owner can make

- Project licence (also unblocks strict Composer validation).
- Official product domain (installer/docs URLs).
- Release signing key and where the signed APT repository is hosted.
- A separate VM for the restore drill.

## Working here

- Panel: `cd panel && php artisan test --compact`, `vendor/bin/pint`.
- Agent (Rust only builds in the Lima VM): `limactl start hostnur-rust`, then
  `limactl shell hostnur-rust -- bash -lc 'cd "<repo>/agent" && cargo clippy --all-targets -- -D warnings && cargo test'`.
  Root-only `*_vm` suites are `#[ignore]`d and need a root VM run.
- Contract: `contracts/v1/catalog.json` is shared by both sides; change it with
  both the agent dispatch and the panel caller.
- Test host: `docs/p16-test-host.md` (rebuild runbook; droplet is recreated per
  session).
