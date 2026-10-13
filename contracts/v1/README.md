# Hostnur agent contract v1

Versioned contract between the PHP panel and any agent implementation (the fake
adapter now, the Rust agent later). The panel owns request IDs and UUIDs; the
agent only ever resolves managed resource IDs from these requests.

## Layout

- `catalog.json` — supported protocol versions and the initial operation catalog
  (per-operation resource kind, parameter schema and retry rule).
- `schemas/` — JSON Schema (2020-12) for the request envelope and each response
  type.
- `fixtures/valid/` — valid request and response examples.
- `fixtures/invalid/` — invalid examples plus `manifest.json` mapping each request
  fixture to its expected machine error code.
- `fixtures/fake/` — deterministic fake-adapter data (server snapshot).

The PHP types live in `panel/app/Agent/Dto` and `panel/app/Contracts/AgentClient`;
`OperationRequestValidator` enforces the request envelope. Contract tests validate
every fixture against both the JSON Schema and the PHP DTOs, so the schemas and
types stay aligned.

## Request envelope

`protocol_version`, `request_id` (UUID), `idempotency_key` (UUID), `operation`,
optional `resource` (`{kind,id}`), `parameters` (operation-specific object) and
optional `timeout_seconds`. Extra keys, invalid UUIDs, wrong types, unknown
operations/parameters, unsupported protocol versions and oversized messages are
rejected.

## Responses

`OperationReceipt` (acceptance only), `OperationSnapshot` (durable state,
progress, bounded events, result/error), `CancellationReceipt`,
`CapabilitySnapshot` and `ServerSnapshot`. Acceptance never means completion;
cancellation is reported as requested until confirmed.

## Error codes

`agent_unavailable`, `unsupported_operation`, `unsupported_protocol`,
`invalid_parameters`, `resource_not_found`, `conflict`, `timeout`,
`execution_unknown`, `permission_denied`, `operation_failed`. Failures return a
stable machine code plus a safe user message; raw secrets, environments and
exception traces are never included.

## Modes and failure behavior

`HOSTNUR_AGENT_MODE` selects the adapter:

- `unavailable` (default) reports no capabilities/observations and fails every
  mutation closed with `agent_unavailable`.
- `real` reports unavailable until phase 11 implements transport.
- `fake` is allowed only in `local`/`testing`, is labelled `mode=fake` on
  capabilities/snapshots, and rejects production use. The fake adapter persists
  executions in `fake_agent_executions` and derives progress/outcome from the
  record plus a controlled clock with scenarios `success`/`failure`/`timeout`
  (`HOSTNUR_FAKE_SCENARIO`); duplicate submissions with the same idempotency key
  reuse one execution.

Unsupported versions/capabilities fail clearly (`unsupported_protocol` /
`unsupported_operation`) rather than guessing. Rust compatibility is validated in
phase 11; this document describes the frozen initial shape.
