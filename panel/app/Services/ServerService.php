<?php

namespace App\Services;

use App\Agent\AgentContractException;
use App\Agent\Dto\OperationRequest;
use App\Agent\Dto\ResourceRef;
use App\Agent\Dto\ServerSnapshot;
use App\Agent\Exceptions\AgentRemoteException;
use App\Agent\Exceptions\AgentTransportException;
use App\Agent\Exceptions\AgentUnavailableException;
use App\Agent\OperationCatalog;
use App\Contracts\AgentClient;
use App\Enums\AgentMode;
use App\Enums\AuditAction;
use App\Enums\AuditOutcome;
use App\Enums\ResourceKind;
use App\Enums\ServiceAction;
use App\Models\ApprovedPackage;
use App\Models\Task;
use App\Models\User;
use Illuminate\Support\Facades\Log;
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
        try {
            return $this->agent->snapshot();
        } catch (AgentTransportException|AgentUnavailableException|AgentRemoteException|AgentContractException $exception) {
            Log::warning('Server snapshot unavailable', ['exception' => $exception::class, 'message' => $exception->getMessage()]);

            return new ServerSnapshot(
                protocolVersion: (int) config('hostnur.protocol_version', 1),
                mode: AgentMode::Unavailable,
                available: false,
                sampledAt: now()->toIso8601String(),
                metrics: [],
                services: [],
                reason: 'The agent is unavailable or busy; retry shortly.',
            );
        }
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
            timeoutSeconds: 120,
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

        $approved = in_array($packageId, config('hostnur.server.packages', []), true)
            || ApprovedPackage::query()->where('name', $packageId)->exists();

        if (! $approved) {
            throw ValidationException::withMessages(['package' => 'Select a package from the approved catalog.']);
        }

        $task = $this->tasks->requestApproved($actor, new OperationRequest(
            protocolVersion: (int) config('hostnur.protocol_version', 1),
            requestId: (string) Str::uuid(),
            idempotencyKey: $idempotencyKey,
            operation: 'package.install',
            resource: null,
            parameters: ['package' => $packageId],
            timeoutSeconds: 900,
        ));

        $this->audit->record($actor, AuditAction::ServerPackageInstallRequested, AuditOutcome::Requested, [
            'resource_kind' => 'package',
            'resource_id' => null,
            'task_id' => $task->getKey(),
            'metadata' => ['operation' => 'package.install'],
        ]);

        return $task;
    }

    public function addFirewallRule(User $actor, int $port, string $protocol, string $source, string $action, string $idempotencyKey): Task
    {
        $this->assertAgentAvailable();
        $this->assertRule($port, $protocol, $source, $action);

        $task = $this->tasks->requestApproved($actor, new OperationRequest(
            protocolVersion: (int) config('hostnur.protocol_version', 1),
            requestId: (string) Str::uuid(),
            idempotencyKey: $idempotencyKey,
            operation: 'firewall.rule.add',
            resource: null,
            parameters: ['port' => $port, 'protocol' => $protocol, 'source' => $source, 'action' => $action],
            timeoutSeconds: null,
        ));

        $this->audit->record($actor, AuditAction::FirewallRuleAdded, AuditOutcome::Requested, [
            'resource_kind' => 'firewall',
            'resource_id' => null,
            'task_id' => $task->getKey(),
            'metadata' => ['operation' => 'firewall.rule.add', 'port' => $port],
        ]);

        return $task;
    }

    public function removeFirewallRule(User $actor, int $port, string $protocol, string $source, string $action, string $idempotencyKey): Task
    {
        $this->assertAgentAvailable();
        $this->assertRule($port, $protocol, $source, $action);

        $task = $this->tasks->requestApproved($actor, new OperationRequest(
            protocolVersion: (int) config('hostnur.protocol_version', 1),
            requestId: (string) Str::uuid(),
            idempotencyKey: $idempotencyKey,
            operation: 'firewall.rule.remove',
            resource: null,
            parameters: ['port' => $port, 'protocol' => $protocol, 'source' => $source, 'action' => $action],
            timeoutSeconds: null,
        ));

        $this->audit->record($actor, AuditAction::FirewallRuleRemoved, AuditOutcome::Requested, [
            'resource_kind' => 'firewall',
            'resource_id' => null,
            'task_id' => $task->getKey(),
            'metadata' => ['operation' => 'firewall.rule.remove', 'port' => $port],
        ]);

        return $task;
    }

    public function setWebHttpsOnly(User $actor, bool $httpsOnly, string $idempotencyKey): Task
    {
        $this->assertAgentAvailable();

        $task = $this->tasks->requestApproved($actor, new OperationRequest(
            protocolVersion: (int) config('hostnur.protocol_version', 1),
            requestId: (string) Str::uuid(),
            idempotencyKey: $idempotencyKey,
            operation: 'firewall.web',
            resource: null,
            parameters: ['https_only' => $httpsOnly],
            timeoutSeconds: null,
        ));

        $this->audit->record($actor, AuditAction::FirewallWebChanged, AuditOutcome::Requested, [
            'resource_kind' => 'firewall',
            'resource_id' => null,
            'task_id' => $task->getKey(),
            'metadata' => ['operation' => 'firewall.web', 'https_only' => $httpsOnly],
        ]);

        return $task;
    }

    public function upgradePackages(User $actor, bool $securityOnly, string $idempotencyKey): Task
    {
        $this->assertAgentAvailable();

        $task = $this->tasks->requestApproved($actor, new OperationRequest(
            protocolVersion: (int) config('hostnur.protocol_version', 1),
            requestId: (string) Str::uuid(),
            idempotencyKey: $idempotencyKey,
            operation: 'package.upgrade',
            resource: null,
            parameters: ['security' => $securityOnly],
            timeoutSeconds: 1800,
        ));

        $this->audit->record($actor, AuditAction::ServerPackageUpgradeRequested, AuditOutcome::Requested, [
            'resource_kind' => 'package',
            'resource_id' => null,
            'task_id' => $task->getKey(),
            'metadata' => ['operation' => 'package.upgrade', 'security_only' => $securityOnly],
        ]);

        return $task;
    }

    private function assertRule(int $port, string $protocol, string $source, string $action): void
    {
        if ($port < 1 || $port > 65535) {
            throw ValidationException::withMessages(['port' => 'Port must be between 1 and 65535.']);
        }

        if (in_array($port, [80, 443], true)) {
            throw ValidationException::withMessages(['port' => 'Ports 80 and 443 are managed automatically and cannot be changed here.']);
        }

        if (! in_array($protocol, ['tcp', 'udp'], true)) {
            throw ValidationException::withMessages(['protocol' => 'Protocol must be tcp or udp.']);
        }

        if (! in_array($action, ['allow', 'deny'], true)) {
            throw ValidationException::withMessages(['action' => 'Action must be allow or deny.']);
        }

        if ($source !== 'any' && ($source === '' || strlen($source) > 64 || preg_match('#^[A-Za-z0-9.:_/-]+$#', $source) !== 1)) {
            throw ValidationException::withMessages(['source' => 'Source must be "any" or an IP/CIDR.']);
        }
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
