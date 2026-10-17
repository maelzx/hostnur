<?php

namespace App\Services;

use App\Agent\AgentContractException;
use App\Agent\Dto\OperationRequest;
use App\Agent\Dto\ResourceRef;
use App\Agent\Exceptions\AgentRemoteException;
use App\Agent\Exceptions\AgentUnavailableException;
use App\Agent\OperationCatalog;
use App\Contracts\AgentClient;
use App\Enums\AuditAction;
use App\Enums\AuditOutcome;
use App\Enums\ErrorCode;
use App\Enums\ResourceKind;
use App\Enums\TaskEventLevel;
use App\Enums\TaskStatus;
use App\Exceptions\TaskNotAuthorizedException;
use App\Jobs\RunTask;
use App\Models\Task;
use App\Models\User;
use App\Support\Redactor;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Str;
use Throwable;

/**
 * The single orchestration path for panel tasks: request, approve, deny, cancel,
 * retry, submit and inspect. UI components and later AI requests must go through
 * here and never call AgentClient directly.
 */
final class TaskService
{
    public function __construct(
        private readonly OperationCatalog $catalog,
        private readonly AgentClient $agent,
        private readonly AuditRecorder $audit,
    ) {}

    public function request(User $actor, OperationRequest $request): Task
    {
        $definition = $this->catalog->definition($request->operation);

        if ($definition === null) {
            throw AgentContractException::unsupportedOperation(
                "Operation {$request->operation} is not in the catalog."
            );
        }

        $fingerprint = $this->fingerprint($request);
        $requiresApproval = $this->requiresApproval($definition);

        $task = DB::transaction(function () use ($actor, $request, $fingerprint, $requiresApproval): Task {
            $existing = Task::query()
                ->where('actor_id', $actor->getKey())
                ->where('idempotency_key', $request->idempotencyKey)
                ->first();

            if ($existing !== null) {
                if (! hash_equals($existing->payload_fingerprint, $fingerprint)) {
                    throw AgentContractException::conflict(
                        'The idempotency key was already used for a different payload.'
                    );
                }

                return $existing;
            }

            $task = Task::create([
                'actor_id' => $actor->getKey(),
                'actor_label' => $actor->email,
                'operation' => $request->operation,
                'resource_kind' => $request->resource?->kind->value,
                'resource_id' => $request->resource?->id,
                'mode' => (string) config('hostnur.agent_mode', 'unavailable'),
                'status' => $requiresApproval ? TaskStatus::AwaitingApproval : TaskStatus::Queued,
                'idempotency_key' => $request->idempotencyKey,
                'parameters' => $request->parameters,
                'payload_fingerprint' => $fingerprint,
                'requested_at' => now(),
                'queued_at' => $requiresApproval ? null : now(),
                'deadline_at' => $request->timeoutSeconds !== null ? now()->addSeconds($request->timeoutSeconds) : null,
            ]);

            $this->audit->forTask($actor, AuditAction::TaskRequested, AuditOutcome::Requested, $task);
            $task->recordEvent('requested', TaskEventLevel::Info, "Requested {$request->operation}.");

            if ($requiresApproval) {
                $task->recordEvent('awaiting_approval', TaskEventLevel::Info, 'Awaiting explicit approval.');
            }

            return $task;
        });

        if ($task->wasRecentlyCreated && $task->status === TaskStatus::Queued) {
            RunTask::dispatch($task->getKey());
        }

        return $task;
    }

    /**
     * Request and, for a consequential operation, immediately approve it: the UI
     * confirmation plus recent authentication is the approval. Used by the
     * administration request services so they share the one task pipeline.
     */
    public function requestApproved(User $actor, OperationRequest $request): Task
    {
        $task = $this->request($actor, $request);

        if ($task->status === TaskStatus::AwaitingApproval) {
            $task = $this->approve($actor, $task);
        }

        return $task;
    }

    public function approve(User $approver, Task $task): Task
    {
        if ($task->status !== TaskStatus::AwaitingApproval) {
            throw new TaskNotAuthorizedException('Only tasks awaiting approval can be approved.');
        }

        $task->forceFill([
            'approved_by' => $approver->getKey(),
            'approved_fingerprint' => $task->payload_fingerprint,
            'approved_at' => now(),
            'approval_expires_at' => now()->addMinutes((int) config('hostnur.tasks.approval_ttl_minutes', 10)),
            'approved_mfa_version' => $approver->mfa_version,
        ]);

        $task->transitionTo(TaskStatus::Queued);
        $task->queued_at = now();
        $task->save();

        $this->audit->forTask($approver, AuditAction::TaskApproved, AuditOutcome::Success, $task);
        $task->recordEvent('approved', TaskEventLevel::Info, "Approved by {$approver->email}.");

        RunTask::dispatch($task->getKey());

        return $task;
    }

    public function deny(User $approver, Task $task, string $reasonCode): Task
    {
        if ($task->status !== TaskStatus::AwaitingApproval) {
            throw new TaskNotAuthorizedException('Only tasks awaiting approval can be denied.');
        }

        $task->forceFill(['denied_reason_code' => $reasonCode, 'finished_at' => now()]);
        $task->transitionTo(TaskStatus::Cancelled);
        $task->save();

        $this->audit->forTask($approver, AuditAction::TaskDenied, AuditOutcome::Denied, $task, ['reason' => $reasonCode]);
        $task->recordEvent('denied', TaskEventLevel::Warning, 'The request was denied.', ['reason' => $reasonCode]);

        return $task;
    }

    public function cancel(User $actor, Task $task): Task
    {
        if ($task->status === TaskStatus::Pending || $task->status === TaskStatus::AwaitingApproval) {
            $task->transitionTo(TaskStatus::Cancelled);
            $task->finished_at = now();
            $task->save();

            $this->audit->forTask($actor, AuditAction::TaskCancelled, AuditOutcome::Success, $task);
            $task->recordEvent('cancelled', TaskEventLevel::Info, 'Cancelled before execution.');

            return $task;
        }

        if ($task->status === TaskStatus::Queued || $task->status === TaskStatus::Running) {
            if ($task->cancel_requested_at === null) {
                $task->cancel_requested_at = now();
                $task->save();

                $this->audit->forTask($actor, AuditAction::TaskCancelRequested, AuditOutcome::Requested, $task);
                $task->recordEvent('cancel_requested', TaskEventLevel::Warning, 'Cancellation requested.');
            }
        }

        return $task;
    }

    public function retry(User $actor, Task $task, string $idempotencyKey): Task
    {
        if (! $task->isTerminal() && $task->status !== TaskStatus::Unknown) {
            throw new TaskNotAuthorizedException('Only finished tasks can be retried.');
        }

        $definition = $this->catalog->definition($task->operation);
        $requiresApproval = $definition !== null && $this->requiresApproval($definition);

        $new = Task::create([
            'actor_id' => $actor->getKey(),
            'actor_label' => $actor->email,
            'operation' => $task->operation,
            'resource_kind' => $task->resource_kind,
            'resource_id' => $task->resource_id,
            'config_revision' => $task->config_revision,
            'mode' => (string) config('hostnur.agent_mode', 'unavailable'),
            'status' => $requiresApproval ? TaskStatus::AwaitingApproval : TaskStatus::Queued,
            'idempotency_key' => $idempotencyKey,
            'parameters' => $task->parameters ?? [],
            'payload_fingerprint' => $task->payload_fingerprint,
            'requested_at' => now(),
            'queued_at' => $requiresApproval ? null : now(),
        ]);

        $this->audit->forTask($actor, AuditAction::TaskRetried, AuditOutcome::Requested, $new, ['original_task_id' => $task->getKey()]);
        $new->recordEvent('retry', TaskEventLevel::Info, "Retry of task {$task->getKey()}.", ['original_task_id' => $task->getKey()]);

        if ($new->status === TaskStatus::Queued) {
            RunTask::dispatch($new->getKey());
        }

        return $new;
    }

    public function run(string $taskId): void
    {
        $task = Task::query()->find($taskId);

        if ($task === null || $task->status->isTerminal()) {
            return;
        }

        if (! $this->claim($task)) {
            return;
        }

        $task->refresh();

        try {
            if ($this->handleCancellation($task)) {
                return;
            }

            if ($task->status === TaskStatus::Queued) {
                if ($this->requiresApprovalForTask($task) && ! $this->approvalValid($task)) {
                    $task->transitionTo(TaskStatus::AwaitingApproval);
                    $task->queued_at = null;
                    $task->save();
                    $this->audit->forTask($task->actor, AuditAction::TaskApprovalExpired, AuditOutcome::Denied, $task);
                    $task->recordEvent('approval_expired', TaskEventLevel::Warning, 'Approval is missing or expired; awaiting approval again.');

                    return;
                }

                $this->authorizeSubmission($task);
                $this->submit($task);
            }

            if ($task->status === TaskStatus::Running) {
                $this->poll($task);
            }
        } catch (Throwable $exception) {
            $this->fail($task, $exception);
        } finally {
            $this->release($task);
        }
    }

    /**
     * @return array{dispatched: int, polled: int, leases_expired: int}
     */
    public function reconcile(): array
    {
        $now = now();

        $leasesExpired = Task::query()
            ->whereNotNull('lease_expires_at')
            ->where('lease_expires_at', '<', $now)
            ->update(['lease_token' => null, 'lease_expires_at' => null]);

        $dispatchable = Task::query()
            ->whereIn('status', [TaskStatus::Queued->value, TaskStatus::Running->value])
            ->where(function ($query) use ($now): void {
                $query->whereNull('lease_expires_at')->orWhere('lease_expires_at', '<', $now);
            })
            ->pluck('id');

        $dispatched = 0;
        $polled = 0;

        foreach ($dispatchable as $id) {
            RunTask::dispatch((string) $id);
            $dispatched++;
        }

        return ['dispatched' => $dispatched, 'polled' => $polled, 'leases_expired' => $leasesExpired];
    }

    private function handleCancellation(Task $task): bool
    {
        if ($task->cancel_requested_at === null) {
            return false;
        }

        if (in_array($task->status, [TaskStatus::Pending, TaskStatus::AwaitingApproval, TaskStatus::Queued], true)) {
            $task->transitionTo(TaskStatus::Cancelled);
            $task->finished_at = now();
            $task->save();

            $this->audit->forTask($task->actor, AuditAction::TaskCancelled, AuditOutcome::Success, $task);
            $task->recordEvent('cancelled', TaskEventLevel::Info, 'Cancelled before submission.');

            return true;
        }

        if ($task->status === TaskStatus::Running && $task->execution_id !== null) {
            try {
                $this->agent->cancel($task->execution_id);
            } catch (Throwable) {
                // The next inspection reconciles the real state.
            }
        }

        return false;
    }

    private function authorizeSubmission(Task $task): void
    {
        $actor = $task->actor;

        if ($actor === null || $actor->is_admin !== true) {
            throw new TaskNotAuthorizedException('The requesting administrator is no longer authorized.');
        }
    }

    private function submit(Task $task): void
    {
        $definition = $this->catalog->definition($task->operation);

        if ($definition === null) {
            throw AgentContractException::unsupportedOperation("Operation {$task->operation} is not in the catalog.");
        }

        $request = new OperationRequest(
            protocolVersion: (int) config('hostnur.protocol_version', 1),
            requestId: (string) Str::uuid(),
            idempotencyKey: $task->idempotency_key,
            operation: $task->operation,
            resource: $task->resource_kind !== null && $task->resource_id !== null
                ? new ResourceRef(ResourceKind::from($task->resource_kind), $task->resource_id)
                : null,
            parameters: $task->parameters ?? [],
            timeoutSeconds: null,
        );

        $receipt = $this->agent->submit($request);

        $task->forceFill(['execution_id' => $receipt->executionId]);
        $task->transitionTo(TaskStatus::Running);
        $task->started_at ??= now();
        $task->save();

        $this->audit->forTask($task->actor, AuditAction::TaskStarted, AuditOutcome::Success, $task, ['execution_id' => $receipt->executionId]);
        $task->recordEvent('submitted', TaskEventLevel::Info, "Submitted to the agent in {$task->mode} mode.", ['mode' => $task->mode]);
    }

    private function poll(Task $task): void
    {
        if ($task->execution_id === null) {
            return;
        }

        try {
            $snapshot = $this->agent->inspect($task->execution_id);
        } catch (Throwable $exception) {
            $code = match (true) {
                $exception instanceof AgentRemoteException => $exception->errorCode(),
                $exception instanceof AgentContractException => $exception->errorCode,
                default => null,
            };

            if ($code === ErrorCode::ExecutionUnknown) {
                $task->transitionTo(TaskStatus::Unknown);
                $task->finished_at ??= now();
                $task->error_code = $code->value;
                $task->error_message = Redactor::message($exception->getMessage());
                $task->save();

                $this->audit->forTask($task->actor, AuditAction::TaskUnknown, AuditOutcome::Failure, $task);
                $task->recordEvent('unknown', TaskEventLevel::Warning, 'Execution outcome is unknown and needs investigation.', ['code' => $code->value]);
            }

            return;
        }

        $this->appendEvents($task, $snapshot->events);

        $target = TaskStatus::fromOperation($snapshot->status);

        if ($target !== $task->status && $task->status->canTransitionTo($target)) {
            $task->transitionTo($target);
        }

        if ($snapshot->error !== null) {
            $task->error_code = $snapshot->error->code->value;
            $task->error_message = Redactor::message($snapshot->error->message);
        }

        if ($snapshot->result !== null) {
            $task->result = $snapshot->result;
        }

        if ($task->status->isTerminal()) {
            $task->finished_at ??= now();
        }

        $task->save();

        $this->auditTerminal($task);
    }

    /**
     * @param  list<array{sequence: int, at: string, message: string}>  $events
     */
    private function appendEvents(Task $task, array $events): void
    {
        $lastSequence = (int) $task->events()->max('sequence');
        $max = (int) config('hostnur.tasks.max_events_per_task', 200);
        $truncated = false;

        foreach ($events as $event) {
            if ($event['sequence'] <= $lastSequence) {
                continue;
            }

            if ($task->events()->count() >= $max) {
                if (! $truncated) {
                    $task->recordEvent('truncated', TaskEventLevel::Warning, 'Further output was truncated.');
                    $truncated = true;
                }

                continue;
            }

            $task->recordEvent('output', TaskEventLevel::Info, (string) $event['message']);
        }
    }

    private function auditTerminal(Task $task): void
    {
        $mapping = [
            TaskStatus::Succeeded->value => [AuditAction::TaskSucceeded, AuditOutcome::Success],
            TaskStatus::Failed->value => [AuditAction::TaskFailed, AuditOutcome::Failure],
            TaskStatus::Cancelled->value => [AuditAction::TaskCancelled, AuditOutcome::Success],
        ];

        if (isset($mapping[$task->status->value])) {
            [$action, $outcome] = $mapping[$task->status->value];
            $this->audit->forTask($task->actor, $action, $outcome, $task);
        }
    }

    private function fail(Task $task, Throwable $exception): void
    {
        if (! $task->status->canTransitionTo(TaskStatus::Failed)) {
            return;
        }

        $task->transitionTo(TaskStatus::Failed);
        $task->finished_at ??= now();

        $code = match (true) {
            $exception instanceof AgentUnavailableException => $exception->errorCode(),
            $exception instanceof AgentRemoteException => $exception->errorCode(),
            $exception instanceof AgentContractException => $exception->errorCode,
            $exception instanceof TaskNotAuthorizedException => $exception->errorCode(),
            default => ErrorCode::OperationFailed,
        };

        $task->error_code = $code->value;
        $task->error_message = Redactor::message($exception->getMessage());
        $task->save();

        $this->audit->forTask($task->actor, AuditAction::TaskFailed, AuditOutcome::Failure, $task, ['code' => $code->value]);
        $task->recordEvent('failed', TaskEventLevel::Error, "Task failed: {$code->value}.", ['code' => $code->value]);
    }

    private function claim(Task $task): bool
    {
        $now = now();
        $token = (string) Str::uuid();
        $expires = $now->copy()->addSeconds((int) config('hostnur.tasks.lease_seconds', 60));

        $updated = Task::query()
            ->whereKey($task->getKey())
            ->whereIn('status', [TaskStatus::Queued->value, TaskStatus::Running->value, TaskStatus::Unknown->value])
            ->where(function ($query) use ($now): void {
                $query->whereNull('lease_expires_at')->orWhere('lease_expires_at', '<', $now);
            })
            ->update([
                'lease_token' => $token,
                'lease_expires_at' => $expires,
                'updated_at' => $now,
            ]);

        return $updated === 1;
    }

    private function release(Task $task): void
    {
        $task->refresh();

        if ($task->lease_token === null) {
            return;
        }

        Task::query()
            ->whereKey($task->getKey())
            ->where('lease_token', $task->lease_token)
            ->update(['lease_token' => null, 'lease_expires_at' => null]);
    }

    private function requiresApprovalForTask(Task $task): bool
    {
        $definition = $this->catalog->definition($task->operation);

        return $definition !== null && $this->requiresApproval($definition);
    }

    /**
     * @param  array<string, mixed>  $definition
     */
    private function requiresApproval(array $definition): bool
    {
        return ($definition['read_only'] ?? false) !== true;
    }

    private function approvalValid(Task $task): bool
    {
        $actor = $task->actor;

        if ($task->approved_fingerprint === null || $task->approval_expires_at === null) {
            return false;
        }

        if (! hash_equals($task->approved_fingerprint, $task->payload_fingerprint)) {
            return false;
        }

        if (now()->greaterThan($task->approval_expires_at)) {
            return false;
        }

        return $actor !== null && (int) $task->approved_mfa_version === (int) $actor->mfa_version;
    }

    private function fingerprint(OperationRequest $request): string
    {
        return hash('sha256', json_encode([
            'operation' => $request->operation,
            'resource' => $request->resource?->toArray(),
            'parameters' => $request->parameters,
        ], JSON_THROW_ON_ERROR));
    }
}
