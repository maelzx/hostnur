<?php

namespace App\Services;

use App\Agent\Dto\OperationRequest;
use App\Agent\Dto\ResourceRef;
use App\Contracts\AgentClient;
use App\Enums\AuditAction;
use App\Enums\AuditOutcome;
use App\Enums\BackupScope;
use App\Enums\RequestState;
use App\Enums\ResourceKind;
use App\Models\Backup;
use App\Models\BackupPolicy;
use App\Models\SourceCredential;
use App\Models\User;
use Cron\CronExpression;
use Illuminate\Support\Collection;
use Illuminate\Support\Str;
use Illuminate\Validation\Rule;
use Illuminate\Validation\ValidationException;

/**
 * Backup policy and simulated requests only. A backup record carries mode and
 * verification status; a fake backup is never labelled recoverable. Restore,
 * encryption-key custody and cross-VM verification belong to phase 19, and the
 * panel never exports its own keys or plaintext secrets.
 */
final class BackupService
{
    public function __construct(
        private readonly TaskService $tasks,
        private readonly AuditRecorder $audit,
        private readonly AgentClient $agent,
    ) {}

    /**
     * @param  array<string, mixed>  $data
     */
    public function savePolicy(User $actor, array $data): BackupPolicy
    {
        $validated = validator($data, [
            'scope' => ['required', Rule::enum(BackupScope::class)],
            'application_id' => ['nullable', 'uuid', Rule::exists('applications', 'id')],
            'expression' => ['required', 'string', 'max:255'],
            'timezone' => ['required', 'string', 'max:64'],
            'retention_days' => ['required', 'integer', 'min:1', 'max:3650'],
            'destination_credential_id' => ['nullable', 'uuid', Rule::exists('source_credentials', 'id')],
            'enabled' => ['sometimes', 'boolean'],
        ])->validate();

        if ($validated['scope'] === BackupScope::Application->value && empty($validated['application_id'])) {
            throw ValidationException::withMessages(['application_id' => 'Select an application for an application-scoped policy.']);
        }

        if (! CronExpression::isValidExpression($validated['expression'])) {
            throw ValidationException::withMessages(['expression' => 'Enter a valid cron expression.']);
        }

        if (! in_array($validated['timezone'], timezone_identifiers_list(), true)) {
            throw ValidationException::withMessages(['timezone' => 'Enter a valid timezone identifier.']);
        }

        if (BackupPolicy::query()->count() >= (int) config('hostnur.server.max_backup_policies', 10)) {
            throw ValidationException::withMessages(['scope' => 'The backup policy limit has been reached.']);
        }

        $policy = BackupPolicy::create([
            'application_id' => $validated['application_id'] ?? null,
            'scope' => BackupScope::from($validated['scope']),
            'expression' => $validated['expression'],
            'timezone' => $validated['timezone'],
            'retention_days' => $validated['retention_days'],
            'destination_credential_id' => $validated['destination_credential_id'] ?? null,
            'enabled' => $validated['enabled'] ?? true,
        ]);

        $this->audit->record($actor, AuditAction::BackupPolicyCreated, AuditOutcome::Success, [
            'resource_kind' => 'application',
            'resource_id' => $policy->application_id,
            'metadata' => ['operation' => 'backup.policy'],
        ]);

        return $policy;
    }

    public function deletePolicy(User $actor, BackupPolicy $policy): void
    {
        $policy->delete();
    }

    public function requestBackup(User $actor, BackupPolicy $policy, string $idempotencyKey): Backup
    {
        $backup = Backup::create([
            'backup_policy_id' => $policy->getKey(),
            'application_id' => $policy->application_id,
            'mode' => (string) config('hostnur.agent_mode', 'unavailable'),
            'state' => RequestState::Requested,
            'verification_status' => 'unverified',
        ]);

        if (config('hostnur.agent_mode') === 'real') {
            $this->agent->registerBackup(new OperationRequest(
                protocolVersion: (int) config('hostnur.protocol_version', 1),
                requestId: (string) Str::uuid(),
                idempotencyKey: (string) Str::uuid(),
                operation: 'agent.register_backup',
                resource: null,
                parameters: array_filter([
                    'backup' => $backup->getKey(),
                    'scope' => $policy->scope === BackupScope::Application ? 'application_data' : 'server',
                    'application' => $policy->application_id,
                ], static fn (mixed $value): bool => $value !== null),
                timeoutSeconds: null,
            ));
        }

        $task = $this->tasks->requestApproved($actor, new OperationRequest(
            protocolVersion: (int) config('hostnur.protocol_version', 1),
            requestId: (string) Str::uuid(),
            idempotencyKey: $idempotencyKey,
            operation: 'backup.create',
            resource: new ResourceRef(ResourceKind::Backup, $backup->getKey()),
            parameters: ['policy' => $policy->getKey()],
            timeoutSeconds: 1800,
        ));

        $backup->forceFill(['task_id' => $task->getKey()])->save();

        $this->audit->record($actor, AuditAction::BackupRequested, AuditOutcome::Requested, [
            'resource_kind' => 'application',
            'resource_id' => $policy->application_id,
            'metadata' => ['operation' => 'backup.create'],
        ]);

        return $backup;
    }

    public function syncState(Backup $backup): Backup
    {
        if ($backup->task !== null) {
            $updates = ['state' => RequestState::fromTaskStatus($backup->task->status)];

            if ($backup->state === RequestState::Requested && $updates['state'] === RequestState::Succeeded) {
                // A simulated backup is never marked verified/recoverable.
                $updates['verification_status'] = $backup->isSimulated() ? 'simulated' : 'unverified';
            }

            $backup->forceFill($updates)->save();
        }

        return $backup;
    }

    /**
     * @return Collection<int, SourceCredential>
     */
    public function destinationCredentials()
    {
        return SourceCredential::query()->latest('created_at')->get();
    }
}
