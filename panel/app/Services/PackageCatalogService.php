<?php

namespace App\Services;

use App\Agent\Dto\OperationRequest;
use App\Contracts\AgentClient;
use App\Enums\AuditAction;
use App\Enums\AuditOutcome;
use App\Models\ApprovedPackage;
use App\Models\User;
use Illuminate\Support\Str;
use Illuminate\Validation\ValidationException;

/**
 * Operator-managed package catalog beyond the built-in allowlist. Names are
 * validated, stored in the panel, and mirrored into the agent registry so the
 * agent's install policy accepts them.
 */
final class PackageCatalogService
{
    public function __construct(
        private readonly AgentClient $agent,
        private readonly AuditRecorder $audit,
    ) {}

    public function add(User $actor, string $name): ApprovedPackage
    {
        $name = strtolower(trim($name));

        if (preg_match('/^[a-z0-9][a-z0-9+.-]{0,63}$/', $name) !== 1) {
            throw ValidationException::withMessages(['name' => 'Package names use lowercase letters, digits, and + . - only.']);
        }

        if (in_array($name, config('hostnur.server.packages', []), true)) {
            throw ValidationException::withMessages(['name' => 'That package is already in the built-in catalog.']);
        }

        if (config('hostnur.agent_mode') === 'real') {
            $this->agent->registerPackage(new OperationRequest(
                protocolVersion: (int) config('hostnur.protocol_version', 1),
                requestId: (string) Str::uuid(),
                idempotencyKey: (string) Str::uuid(),
                operation: 'agent.register_package',
                resource: null,
                parameters: ['package' => $name],
                timeoutSeconds: null,
            ));
        }

        $package = ApprovedPackage::query()->firstOrCreate(['name' => $name]);

        $this->audit->record($actor, AuditAction::PackageApproved, AuditOutcome::Success, [
            'resource_kind' => 'package',
            'resource_id' => null,
            'metadata' => ['operation' => 'package.approve', 'package' => $name],
        ]);

        return $package;
    }

    public function remove(User $actor, ApprovedPackage $package): void
    {
        $name = $package->name;

        if (config('hostnur.agent_mode') === 'real') {
            $this->agent->removePackage(new OperationRequest(
                protocolVersion: (int) config('hostnur.protocol_version', 1),
                requestId: (string) Str::uuid(),
                idempotencyKey: (string) Str::uuid(),
                operation: 'agent.remove_package',
                resource: null,
                parameters: ['package' => $name],
                timeoutSeconds: null,
            ));
        }

        $package->delete();

        $this->audit->record($actor, AuditAction::PackageRemoved, AuditOutcome::Success, [
            'resource_kind' => 'package',
            'resource_id' => null,
            'metadata' => ['operation' => 'package.remove', 'package' => $name],
        ]);
    }
}
