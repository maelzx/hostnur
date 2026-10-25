<?php

namespace App\Services;

use App\Actions\Deployments\DeploymentPreflight;
use App\Agent\Dto\OperationRequest;
use App\Agent\Dto\ResourceRef;
use App\Contracts\AgentClient;
use App\Enums\AuditAction;
use App\Enums\AuditOutcome;
use App\Enums\DeploymentState;
use App\Enums\ResourceKind;
use App\Enums\TaskStatus;
use App\Exceptions\DeploymentPreflightException;
use App\Models\Application;
use App\Models\Deployment;
use App\Models\User;
use Illuminate\Support\Collection;
use Illuminate\Support\Str;
use Illuminate\Validation\ValidationException;

final class DeploymentService
{
    public function __construct(
        private readonly DeploymentPreflight $preflight,
        private readonly TaskService $tasks,
        private readonly AuditRecorder $audit,
        private readonly AgentClient $agent,
    ) {}

    public function deploy(User $actor, Application $application, string $idempotencyKey, bool $runMigrations, bool $migrationRecoveryAcknowledged = false): Deployment
    {
        if ($runMigrations && ! $migrationRecoveryAcknowledged) {
            throw ValidationException::withMessages([
                'run_migrations' => 'Running database migrations requires confirming a recovery point first.',
            ]);
        }

        $this->assertPreflight($this->preflight->deployChecks($application->refresh()));

        $revision = $application->revisions()->first();

        $deployment = $this->firstOrCreate($actor, $idempotencyKey, $application, 'application.deploy', [
            'revision_id' => $revision?->getKey(),
            'revision_number' => $revision?->revision,
            'run_migrations' => $runMigrations,
            'migration_acknowledged_at' => $runMigrations ? now() : null,
        ]);

        if ($deployment->wasRecentlyCreated) {
            $this->requestTask($actor, $application, $deployment, [
                'revision' => $revision !== null ? (string) $revision->getKey() : null,
                'ref' => $application->git_ref,
                'run_migrations' => $runMigrations,
                'recovery_acknowledged' => $migrationRecoveryAcknowledged,
            ]);
            $this->audit->record($actor, AuditAction::ApplicationDeployRequested, AuditOutcome::Requested, [
                'resource_kind' => 'application',
                'resource_id' => $application->getKey(),
                'task_id' => $deployment->task_id,
                'metadata' => ['operation' => 'application.deploy'],
            ]);
        }

        return $deployment;
    }

    public function restart(User $actor, Application $application, string $idempotencyKey): Deployment
    {
        $this->assertPreflight($this->preflight->operationChecks($application->refresh(), 'application.restart'));

        $deployment = $this->firstOrCreate($actor, $idempotencyKey, $application, 'application.restart');

        if ($deployment->wasRecentlyCreated) {
            $this->requestTask($actor, $application, $deployment, []);
            $this->audit->record($actor, AuditAction::ApplicationRestartRequested, AuditOutcome::Requested, [
                'resource_kind' => 'application',
                'resource_id' => $application->getKey(),
                'task_id' => $deployment->task_id,
                'metadata' => ['operation' => 'application.restart'],
            ]);
        }

        return $deployment;
    }

    public function start(User $actor, Application $application, string $idempotencyKey): Deployment
    {
        return $this->control($actor, $application, 'application.start', $idempotencyKey, AuditAction::ApplicationStartRequested);
    }

    public function stop(User $actor, Application $application, string $idempotencyKey): Deployment
    {
        return $this->control($actor, $application, 'application.stop', $idempotencyKey, AuditAction::ApplicationStopRequested);
    }

    private function control(User $actor, Application $application, string $operation, string $idempotencyKey, AuditAction $auditAction): Deployment
    {
        $this->assertPreflight($this->preflight->operationChecks($application->refresh(), $operation));

        $deployment = $this->firstOrCreate($actor, $idempotencyKey, $application, $operation);

        if ($deployment->wasRecentlyCreated) {
            $this->requestTask($actor, $application, $deployment, []);
            $this->audit->record($actor, $auditAction, AuditOutcome::Requested, [
                'resource_kind' => 'application',
                'resource_id' => $application->getKey(),
                'task_id' => $deployment->task_id,
                'metadata' => ['operation' => $operation],
            ]);
        }

        return $deployment;
    }

    public function rollback(User $actor, Application $application, string $releaseId, string $idempotencyKey): Deployment
    {
        $this->assertPreflight($this->preflight->operationChecks($application->refresh(), 'application.rollback'));

        $release = Deployment::query()
            ->where('id', $releaseId)
            ->where('application_id', $application->getKey())
            ->whereNotNull('release_reference')
            ->where('state', DeploymentState::Succeeded->value)
            ->first();

        if ($release === null) {
            throw ValidationException::withMessages(['release' => 'Select a known successful release for this application.']);
        }

        $deployment = $this->firstOrCreate($actor, $idempotencyKey, $application, 'application.rollback', [
            'previous_release_reference' => $release->release_reference,
        ]);

        if ($deployment->wasRecentlyCreated) {
            // The agent names releases by its own release id (the deploy result).
            $this->requestTask($actor, $application, $deployment, ['release' => $release->release_reference]);
            $this->audit->record($actor, AuditAction::ApplicationRollbackRequested, AuditOutcome::Requested, [
                'resource_kind' => 'application',
                'resource_id' => $application->getKey(),
                'task_id' => $deployment->task_id,
                'metadata' => ['operation' => 'application.rollback'],
            ]);
        }

        return $deployment;
    }

    /**
     * @return Collection<int, Deployment>
     */
    public function selectableReleases(Application $application): Collection
    {
        return Deployment::query()
            ->where('application_id', $application->getKey())
            ->whereNotNull('release_reference')
            ->where('state', DeploymentState::Succeeded->value)
            ->latest('finished_at')
            ->get()
            ->unique('release_reference')
            ->values();
    }

    public function refresh(Deployment $deployment): Deployment
    {
        $task = $deployment->task;

        if ($task === null) {
            return $deployment;
        }

        $state = $this->mapState($task->status);
        $updates = ['state' => $state];

        if ($task->started_at !== null) {
            $updates['started_at'] = $task->started_at;
        }

        if ($state->isTerminal()) {
            $updates['finished_at'] = $task->finished_at ?? now();
        }

        if ($state === DeploymentState::Succeeded) {
            if ($deployment->operation === 'application.deploy' && $deployment->release_reference === null) {
                // A real deploy reports the release it activated; that id is what
                // a later rollback must name. Simulated runs get a placeholder.
                $release = is_array($task->result) ? ($task->result['release'] ?? null) : null;
                $updates['release_reference'] = is_string($release) && Str::isUuid($release)
                    ? $release
                    : (string) Str::uuid();
            }

            if ($deployment->operation === 'application.rollback' && $deployment->previous_release_reference !== null) {
                $updates['release_reference'] = $deployment->previous_release_reference;
            }

            if (is_array($task->result)) {
                $updates['health_summary'] = $task->result;
            }
        }

        if ($state === DeploymentState::Failed || $state === DeploymentState::Unknown) {
            $updates['error_code'] = $task->error_code ?? $state->value;
            $updates['error_message'] = $task->error_message ?? 'The deployment did not complete successfully.';
        }

        $deployment->forceFill($updates)->save();

        return $deployment;
    }

    /**
     * @param  array<string, mixed>  $attributes
     */
    private function firstOrCreate(User $actor, string $idempotencyKey, Application $application, string $operation, array $attributes = []): Deployment
    {
        return Deployment::query()->firstOrCreate(
            ['idempotency_key' => $idempotencyKey],
            array_merge([
                'application_id' => $application->getKey(),
                'operation' => $operation,
                'git_ref' => $application->git_ref,
                'runtime' => $application->runtime,
                'mode' => (string) config('hostnur.agent_mode', 'unavailable'),
                'state' => DeploymentState::Requested,
                'requested_at' => now(),
                'created_by' => $actor->getKey(),
            ], $attributes),
        );
    }

    /**
     * @param  array<string, mixed>  $parameters
     */
    private function requestTask(User $actor, Application $application, Deployment $deployment, array $parameters): void
    {
        $parameters = array_filter($parameters, fn (mixed $value): bool => $value !== null);

        if ($deployment->operation === 'application.deploy') {
            $this->registerSource($application);
        }

        $request = new OperationRequest(
            protocolVersion: (int) config('hostnur.protocol_version', 1),
            requestId: (string) Str::uuid(),
            idempotencyKey: $deployment->idempotency_key,
            operation: $deployment->operation,
            resource: new ResourceRef(ResourceKind::Application, $application->getKey()),
            parameters: $parameters,
            timeoutSeconds: null,
        );

        $task = $this->tasks->request($actor, $request);

        // The wizard confirmation plus recent authentication is the approval for
        // this consequential operation, so queue it for execution immediately.
        if ($task->status === TaskStatus::AwaitingApproval) {
            $task = $this->tasks->approve($actor, $task);
        }

        $deployment->forceFill(['task_id' => $task->getKey()])->save();
    }

    /**
     * Register the application's source/domain with the agent before submitting a
     * real deployment, so the agent can resolve them from its own registry.
     */
    private function registerSource(Application $application): void
    {
        if (config('hostnur.agent_mode') !== 'real') {
            return;
        }

        $domain = $application->domains()->first();

        if ($application->repository_url === null || $domain === null) {
            throw ValidationException::withMessages([
                'repository_url' => 'A repository URL and a domain are required before deploying.',
            ]);
        }

        $request = new OperationRequest(
            protocolVersion: (int) config('hostnur.protocol_version', 1),
            requestId: (string) Str::uuid(),
            idempotencyKey: (string) Str::uuid(),
            operation: 'agent.register_application',
            resource: null,
            parameters: [
                'application' => $application->getKey(),
                'repository_url' => (string) $application->repository_url,
                'server_name' => (string) $domain->hostname,
                'provider' => (string) $application->provider,
                'runtime' => (string) $application->runtime,
                'runtime_version' => (string) ($application->runtime_version ?: $application->runtime),
                'health_path' => (string) ($application->revisions()->first()?->settings['health_path'] ?? 'up'),
            ],
            timeoutSeconds: null,
        );

        try {
            $this->agent->registerApplication($request);
        } catch (\Throwable $exception) {
            throw ValidationException::withMessages([
                'repository_url' => 'Could not register the source with the agent: '.$exception->getMessage(),
            ]);
        }
    }

    private function mapState(TaskStatus $status): DeploymentState
    {
        return match ($status) {
            TaskStatus::Pending, TaskStatus::AwaitingApproval, TaskStatus::Queued => DeploymentState::Requested,
            TaskStatus::Running => DeploymentState::Running,
            TaskStatus::Succeeded => DeploymentState::Succeeded,
            TaskStatus::Failed => DeploymentState::Failed,
            TaskStatus::Cancelled => DeploymentState::Cancelled,
            TaskStatus::Unknown => DeploymentState::Unknown,
        };
    }

    /**
     * @param  list<array{required: bool, passed: bool, label: string, message: string|null}>  $checks
     */
    private function assertPreflight(array $checks): void
    {
        if ($this->preflight->passes($checks)) {
            return;
        }

        $messages = [];
        foreach ($this->preflight->failures($checks) as $failure) {
            $messages[] = $failure['message'] ?? $failure['label'];
        }

        throw new DeploymentPreflightException(implode(' ', $messages));
    }
}
