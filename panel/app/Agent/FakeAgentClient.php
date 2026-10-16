<?php

namespace App\Agent;

use App\Agent\Dto\ApplicationLogSnapshot;
use App\Agent\Dto\ApplicationMetricsSnapshot;
use App\Agent\Dto\ApplicationObservation;
use App\Agent\Dto\CancellationReceipt;
use App\Agent\Dto\CapabilitySnapshot;
use App\Agent\Dto\OperationReceipt;
use App\Agent\Dto\OperationRequest;
use App\Agent\Dto\OperationSnapshot;
use App\Agent\Dto\ServerSnapshot;
use App\Agent\Dto\ServerUpdatesSnapshot;
use App\Agent\Exceptions\AgentConfigurationException;
use App\Contracts\AgentClient;
use App\Enums\AgentMode;
use App\Enums\CancellationStatus;
use App\Enums\FakeScenario;
use App\Models\FakeAgentExecution;
use Illuminate\Database\UniqueConstraintViolationException;
use Illuminate\Support\Str;
use RuntimeException;

/**
 * Deterministic simulated agent. Read-only snapshots come from checked-in
 * fixtures; submit/inspect/cancel are durable and derived from the persisted
 * record plus a controlled clock. It never touches the network, the filesystem
 * outside the contract fixtures, or any host daemon.
 */
final class FakeAgentClient implements AgentClient
{
    public function __construct(
        private readonly OperationCatalog $catalog,
        private readonly AgentModeGuard $guard,
        private readonly FakeExecutionSimulator $simulator,
    ) {}

    public function capabilities(): CapabilitySnapshot
    {
        $this->guard->assertFakeModeAllowed();

        return new CapabilitySnapshot(
            protocolVersion: $this->protocolVersion(),
            mode: AgentMode::Fake,
            available: true,
            operations: $this->catalog->names(),
            sampledAt: now()->toIso8601String(),
            reason: 'Simulated agent for local development and tests.',
        );
    }

    public function snapshot(): ServerSnapshot
    {
        $this->guard->assertFakeModeAllowed();

        $fixture = $this->fixture('server-snapshot');
        $fixture['sampled_at'] = now()->toIso8601String();

        return ServerSnapshot::fromArray($fixture);
    }

    public function applicationLogs(OperationRequest $request): ApplicationLogSnapshot
    {
        $this->guard->assertFakeModeAllowed();

        $fixture = $this->fixture('application-log');
        $fixture['application_id'] = $this->applicationResourceId($request, $fixture['application_id']);
        $fixture['sampled_at'] = now()->toIso8601String();

        return ApplicationLogSnapshot::fromArray($fixture);
    }

    public function applicationMetrics(OperationRequest $request): ApplicationMetricsSnapshot
    {
        $this->guard->assertFakeModeAllowed();

        $fixture = $this->fixture('application-metrics');
        $fixture['application_id'] = $this->applicationResourceId($request, $fixture['application_id']);
        $fixture['sampled_at'] = now()->toIso8601String();

        return ApplicationMetricsSnapshot::fromArray($fixture);
    }

    public function applicationObservation(OperationRequest $request): ApplicationObservation
    {
        $this->guard->assertFakeModeAllowed();

        $fixture = $this->fixture('application-observation');
        $fixture['application_id'] = $this->applicationResourceId($request, $fixture['application_id']);
        $fixture['sampled_at'] = now()->toIso8601String();

        return ApplicationObservation::fromArray($fixture);
    }

    public function serverUpdates(OperationRequest $request): ServerUpdatesSnapshot
    {
        $this->guard->assertFakeModeAllowed();

        $fixture = $this->fixture('server-updates');
        $fixture['sampled_at'] = now()->toIso8601String();

        return ServerUpdatesSnapshot::fromArray($fixture);
    }

    public function registerPackage(OperationRequest $request): void
    {
        // No durable registry in the fake adapter.
    }

    public function removePackage(OperationRequest $request): void
    {
        // No durable registry in the fake adapter.
    }

    public function registerBackup(OperationRequest $request): void
    {
        // No durable registry in the fake adapter.
    }

    /**
     * @param  array<string, mixed>  $fallback
     */
    private function applicationResourceId(OperationRequest $request, mixed $fallback): string
    {
        if ($request->resource !== null && $request->resource->id !== '') {
            return $request->resource->id;
        }

        return is_string($fallback) ? $fallback : '';
    }

    public function submit(OperationRequest $request): OperationReceipt
    {
        $this->guard->assertFakeModeAllowed();

        $definition = $this->catalog->definition($request->operation);

        if ($definition === null) {
            throw AgentContractException::unsupportedOperation(
                "Operation {$request->operation} is not supported by the fake agent."
            );
        }

        $record = $this->findOrCreate($request, $definition);

        if (! $this->matches($record, $request)) {
            throw AgentContractException::conflict(
                'The idempotency key was already used for a different operation or payload.'
            );
        }

        return $this->simulator->receipt($record);
    }

    public function inspect(string $executionId): OperationSnapshot
    {
        $record = $this->find($executionId);

        return $this->simulator->snapshot($record);
    }

    public function cancel(string $executionId): CancellationReceipt
    {
        $record = $this->find($executionId);

        if ($record->read_only) {
            return $this->cancellation($record, CancellationStatus::Unsupported);
        }

        if ($this->simulator->status($record)->isTerminal()) {
            return $this->cancellation($record, CancellationStatus::AlreadyFinished);
        }

        if ($record->cancel_requested_at === null) {
            $record->forceFill(['cancel_requested_at' => now()])->save();
        }

        return $this->cancellation($record, CancellationStatus::Requested);
    }

    public function registerApplication(OperationRequest $request): void
    {
        // The fake adapter has no durable registry; registration is a no-op.
    }

    public function registerSchedule(OperationRequest $request): void
    {
        // The fake adapter has no durable registry; registration is a no-op.
    }

    public function registerDatabase(OperationRequest $request): array
    {
        // The fake adapter has no durable registry.
        return [];
    }

    public function removeDatabase(OperationRequest $request): void
    {
        // The fake adapter has no durable registry; removal is a no-op.
    }

    public function rotateDatabase(OperationRequest $request): void
    {
        // The fake adapter has no durable registry; rotation is a no-op.
    }

    public function registerCertificate(OperationRequest $request): void
    {
        // The fake adapter has no durable registry; registration is a no-op.
    }

    private function find(string $executionId): FakeAgentExecution
    {
        $record = FakeAgentExecution::query()->where('execution_id', $executionId)->first();

        if ($record === null) {
            throw AgentContractException::executionUnknown("Execution {$executionId} is unknown.");
        }

        return $record;
    }

    /**
     * @param  array<string, mixed>  $definition
     */
    private function findOrCreate(OperationRequest $request, array $definition): FakeAgentExecution
    {
        try {
            return FakeAgentExecution::query()->firstOrCreate(
                ['idempotency_key' => $request->idempotencyKey],
                [
                    'execution_id' => (string) Str::uuid(),
                    'request_id' => $request->requestId,
                    'operation' => $request->operation,
                    'resource_kind' => $request->resource?->kind->value,
                    'resource_id' => $request->resource?->id,
                    'parameters' => $request->parameters,
                    'scenario' => $this->scenario()->value,
                    'read_only' => (bool) ($definition['read_only'] ?? false),
                ],
            );
        } catch (UniqueConstraintViolationException) {
            return FakeAgentExecution::query()
                ->where('idempotency_key', $request->idempotencyKey)
                ->firstOrFail();
        }
    }

    private function matches(FakeAgentExecution $record, OperationRequest $request): bool
    {
        return $record->operation === $request->operation
            && $record->resource_kind === $request->resource?->kind->value
            && $record->resource_id === $request->resource?->id
            && $record->parameters === $request->parameters;
    }

    private function scenario(): FakeScenario
    {
        $configured = (string) config('hostnur.fake.scenario', FakeScenario::Success->value);
        $scenario = FakeScenario::tryFrom($configured);

        if ($scenario === null) {
            throw new AgentConfigurationException("Unknown fake scenario [{$configured}].");
        }

        return $scenario;
    }

    private function cancellation(FakeAgentExecution $record, CancellationStatus $status): CancellationReceipt
    {
        return new CancellationReceipt(
            protocolVersion: $this->protocolVersion(),
            executionId: $record->execution_id,
            status: $status,
            requestedAt: now()->toIso8601String(),
        );
    }

    /**
     * @return array<string, mixed>
     */
    private function fixture(string $name): array
    {
        $path = rtrim((string) config('hostnur.contracts_path'), '/')."/fixtures/fake/{$name}.json";

        if (! is_file($path)) {
            throw new RuntimeException("Fake agent fixture was not found at {$path}.");
        }

        $decoded = json_decode((string) file_get_contents($path), true, flags: JSON_THROW_ON_ERROR);

        if (! is_array($decoded)) {
            throw new RuntimeException("Fake agent fixture {$name} is malformed.");
        }

        return $decoded;
    }

    private function protocolVersion(): int
    {
        return (int) config('hostnur.protocol_version', 1);
    }
}
