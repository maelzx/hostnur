<?php

namespace App\Services;

use App\Agent\Dto\OperationRequest;
use App\Agent\Dto\ResourceRef;
use App\Agent\Dto\ServerSnapshot;
use App\Agent\OperationCatalog;
use App\Contracts\AgentClient;
use App\Enums\AuditAction;
use App\Enums\AuditOutcome;
use App\Enums\ResourceKind;
use App\Enums\ServiceAction;
use App\Models\Task;
use App\Models\User;
use Illuminate\Support\Str;
use Illuminate\Validation\ValidationException;

/**
 * Read-only server observation plus allowlisted service/package requests. Service
 * IDs and package IDs are resolved from the adapter snapshot / approved catalog;
 * arbitrary unit names, package names and flags are never accepted.
 */
final class ServerService
{
    public function __construct(
        private readonly AgentClient $agent,
        private readonly OperationCatalog $catalog,
        private readonly TaskService $tasks,
        private readonly AuditRecorder $audit,
    ) {}

    public function snapshot(): ServerSnapshot
    {
        return $this->agent->snapshot();
    }

    /**
     * @return list<string>
     */
    public function serviceNames(): array
    {
        return array_map(
            fn (array $service): string => $service['name'],
            $this->snapshot()->services,
        );
    }

    public function control(User $actor, string $serviceId, ServiceAction $action, string $idempotencyKey): Task
    {
        $this->assertAgentAvailable();

        if (! in_array($serviceId, $this->serviceNames(), true)) {
            throw ValidationException::withMessages(['service' => 'Select a service reported by the agent.']);
        }

        $task = $this->tasks->requestApproved($actor, new OperationRequest(
            protocolVersion: (int) config('hostnur.protocol_version', 1),
            requestId: (string) Str::uuid(),
            idempotencyKey: $idempotencyKey,
            operation: 'service.control',
            resource: new ResourceRef(ResourceKind::Service, $serviceId),
            parameters: ['action' => $action->value],
            timeoutSeconds: null,
        ));

        $this->audit->record($actor, AuditAction::ServerServiceControlRequested, AuditOutcome::Requested, [
            'resource_kind' => 'service',
            'resource_id' => $serviceId,
            'task_id' => $task->getKey(),
            'metadata' => ['operation' => 'service.control'],
        ]);

        return $task;
    }

    public function installPackage(User $actor, string $packageId, string $idempotencyKey): Task
    {
        $this->assertAgentAvailable();

        if (! in_array($packageId, config('hostnur.server.packages', []), true)) {
            throw ValidationException::withMessages(['package' => 'Select a package from the approved catalog.']);
        }

        $task = $this->tasks->requestApproved($actor, new OperationRequest(
            protocolVersion: (int) config('hostnur.protocol_version', 1),
            requestId: (string) Str::uuid(),
            idempotencyKey: $idempotencyKey,
            operation: 'package.install',
            resource: null,
            parameters: ['package' => $packageId],
            timeoutSeconds: null,
        ));

        $this->audit->record($actor, AuditAction::ServerPackageInstallRequested, AuditOutcome::Requested, [
            'resource_kind' => 'package',
            'resource_id' => null,
            'task_id' => $task->getKey(),
            'metadata' => ['operation' => 'package.install'],
        ]);

        return $task;
    }

    private function assertAgentAvailable(): void
    {
        $capabilities = $this->agent->capabilities();

        if (! $capabilities->available) {
            throw ValidationException::withMessages([
                'agent' => $capabilities->reason ?? 'The agent is unavailable; requests are closed.',
            ]);
        }
    }
}
