# Agent integration map

This is the frozen PHP side of the phase-9 milestone. The future Rust agent must
**independently re-validate** every input; the PHP schema is not the root trust
boundary. Contract sources live in [`../contracts/v1/`](../contracts/v1/):
`catalog.json`, `schemas/*.schema.json` and `fixtures/{valid,invalid,fake}`.

## PHP surface

- `App\Contracts\AgentClient` — `capabilities()`, `snapshot()`, `submit()`,
  `inspect()`, `cancel()`.
- DTOs in `App\Agent\Dto` (request, receipt, operation snapshot, cancellation
  receipt, capability snapshot, server snapshot, error).
- `OperationRequestValidator` enforces the request envelope (extra keys, UUIDs,
  types, protocol allowlist, operation allowlist, resource/parameter schemas,
  bounded message size).
- All panel mutations go through `App\Services\TaskService` (request → approve →
  run/poll → cancel/retry); UI components never call `AgentClient` directly.

## Operation map

| Operation | Resource (ID source) | Parameters | Fixture | Approval / privilege | Recovery |
|---|---|---|---|---|---|
| `agent.capabilities` | none | — | `valid/request-agent-capabilities.json` | read-only; MFA | bounded retry |
| `agent.inspect` | none | `execution_id` uuid | `valid/request-agent-inspect.json` | read-only; MFA | bounded retry |
| `agent.cancel` | none | `execution_id` uuid | `valid/request-agent-cancel.json` | recent auth + task approval | not rollback; `requested` until confirmed |
| `server.snapshot` | none | — | `valid/request-server-snapshot.json` | read-only; MFA | bounded retry |
| `application.deploy` | application (panel registry) | `revision` uuid, `ref` string, `run_migrations` bool | (see request schema) | recent auth + task approval | no blind replay; unknown ≠ success |
| `application.restart` | application | — | `valid/request-application-restart.json` | recent auth + task approval | capability-declared retry |
| `application.rollback` | application | `release` uuid (known app release) | — | recent auth + task approval | only a known compatible release |
| `service.control` | service (agent snapshot) | `action` enum start/stop/restart | `valid/request-service-control.json` | recent auth + task approval | no raw systemd unit |
| `package.install` | none | `package` string (approved catalog ID) | — | recent auth + task approval | not arbitrary APT flags/repos |
| `schedule.apply` | schedule (panel registry) | `revision` uuid | — | recent auth + task approval | logical owner is the app |
| `certificate.issue` | domain (panel registry) | `provider` enum `http01` | — | recent auth + task approval | no wildcard/DNS-01 |
| `certificate.install` | domain (panel registry) | `certificate` string ref | — | recent auth + task approval | installs material staged by `agent.register_certificate` (`certificate_pem`/`private_key`); keys never travel in task parameters |
| `database.provision` | none | `request` uuid (panel registry) | — | recent auth + task approval | panel-generated credentials only |
| `backup.create` | backup (panel registry) | `policy` uuid | — | recent auth + task approval | simulated ≠ recoverable |

Resource IDs are UUIDs owned by the panel; the agent must resolve them against a
managed registry and never trust caller-supplied paths, unit names or hostnames.

## Envelope and errors

Request: `protocol_version` (currently 1), `request_id` (uuid), `idempotency_key`
(uuid), `operation`, optional `resource` (`{kind,id}`), `parameters`, optional
`timeout_seconds`. Responses echo correlation/version and distinguish acceptance
from terminal outcome.

Error codes: `agent_unavailable`, `unsupported_operation`, `unsupported_protocol`,
`invalid_parameters`, `resource_not_found`, `conflict`, `timeout`,
`execution_unknown`, `permission_denied`, `operation_failed`. No secrets, process
environments or exception traces are included.

## Idempotency, cancellation and unknown state

- `submit` is idempotent on `idempotency_key`; the same key with a different payload
  is a `conflict`. Transactions dedupe actor + key; duplicate jobs must be harmless.
- `cancel` reports `requested` / `unsupported` / `already_finished`; cancellation is
  not rollback and is never reported before confirmation.
- Unknown execution (`inspect` of an unknown id) is explicit and is treated as
  `unknown`, not success. A lost receipt is queried/reconciled with the same
  identity, never blindly replayed for a destructive operation.

## Limits and redaction

Bounded message size (1 MiB), parameters (64), events (100), observations (100);
retained task events are capped and redacted (structured allowlist plus secret
patterns) before persistence. Queue jobs and audit records contain only task IDs
and non-secret metadata.

## Capabilities and modes

`capabilities()` reports protocol/mode/availability/operation support and a reason.
`unavailable` (default) reports no capabilities and fails mutations closed; `real`
reports unavailable until phase 11; `fake` is local/testing-only, labelled
`mode=fake`, and never writes real applied state.

## Known gaps (explicitly out of scope for this milestone)

- The real transport now exists (phase 11, P11.1–P11.5): Rust framing, peer-checked
  Unix socket, durable journal and the PHP `RealAgentClient`. Privileged operation
  handlers beyond read-only `server.snapshot` are still unimplemented and return
  `unsupported_operation`; systemd/socket VM gates (P11.6) remain.
- Real consistency/restore, TLS issuance/renewal, package installation and OS
  provisioning happen only through the agent in later phases.
- Domains reach Nginx through deployment (`server_name`) and certificate
  issue/install; there is no separate `domain.apply` operation (removed, session 051).
