<?php

namespace App\Services;

use App\Agent\Dto\OperationRequest;
use App\Agent\Dto\ResourceRef;
use App\Enums\AuditAction;
use App\Enums\AuditOutcome;
use App\Enums\ResourceKind;
use App\Models\Application;
use App\Models\ScheduledJob;
use App\Models\Task;
use App\Models\User;
use Cron\CronExpression;
use Illuminate\Support\Str;
use Illuminate\Validation\ValidationException;

/**
 * Desired schedule configuration only. Saving never installs cron/systemd; the
 * panel's own scheduler runs task maintenance, and hosted commands execute later
 * under the application's Linux user via the agent.
 */
final class ScheduleService
{
    public function __construct(
        private readonly TaskService $tasks,
        private readonly AuditRecorder $audit,
    ) {}

    /**
     * @param  array<string, mixed>  $data
     */
    public function save(User $actor, Application $application, array $data, ?ScheduledJob $job = null): ScheduledJob
    {
        $validated = $this->validate($data);

        if ($job === null && $application->scheduledJobs()->count() >= (int) config('hostnur.server.max_schedules', 20)) {
            throw ValidationException::withMessages(['name' => 'This application has reached its schedule limit.']);
        }

        $attributes = [
            'application_id' => $application->getKey(),
            'name' => $validated['name'],
            'expression' => $validated['expression'],
            'timezone' => $validated['timezone'],
            'command_reference' => $validated['command_reference'],
            'enabled' => $validated['enabled'] ?? true,
        ];

        if ($job === null) {
            $job = ScheduledJob::create($attributes);
            $action = AuditAction::ScheduleCreated;
        } else {
            $job->forceFill($attributes + ['revision' => $job->revision + 1])->save();
            $action = AuditAction::ScheduleUpdated;
        }

        $this->audit->record($actor, $action, AuditOutcome::Success, [
            'resource_kind' => 'application',
            'resource_id' => $application->getKey(),
            'metadata' => ['operation' => 'schedule.save'],
        ]);

        return $job;
    }

    public function delete(User $actor, ScheduledJob $job): void
    {
        $applicationId = $job->application_id;
        $job->delete();

        $this->audit->record($actor, AuditAction::ScheduleDeleted, AuditOutcome::Success, [
            'resource_kind' => 'application',
            'resource_id' => $applicationId,
            'metadata' => ['operation' => 'schedule.delete'],
        ]);
    }

    public function apply(User $actor, ScheduledJob $job, string $idempotencyKey): Task
    {
        $task = $this->tasks->requestApproved($actor, new OperationRequest(
            protocolVersion: (int) config('hostnur.protocol_version', 1),
            requestId: (string) Str::uuid(),
            idempotencyKey: $idempotencyKey,
            operation: 'schedule.apply',
            resource: new ResourceRef(ResourceKind::Schedule, $job->getKey()),
            parameters: ['revision' => (string) $job->revision],
            timeoutSeconds: null,
        ));

        $this->audit->record($actor, AuditAction::ScheduleApplyRequested, AuditOutcome::Requested, [
            'resource_kind' => 'application',
            'resource_id' => $job->application_id,
            'task_id' => $task->getKey(),
            'metadata' => ['operation' => 'schedule.apply'],
        ]);

        return $task;
    }

    /**
     * @param  array<string, mixed>  $data
     * @return array{name: string, expression: string, timezone: string, command_reference: string, enabled: bool}
     */
    private function validate(array $data): array
    {
        $validated = validator($data, [
            'name' => ['required', 'string', 'max:255'],
            'expression' => ['required', 'string', 'max:255'],
            'timezone' => ['required', 'string', 'max:64'],
            'command_reference' => ['required', 'string', 'max:255', 'regex:/^[A-Za-z0-9][A-Za-z0-9 _:.\/-]*$/'],
            'enabled' => ['sometimes', 'boolean'],
        ])->validate();

        if (! CronExpression::isValidExpression($validated['expression'])) {
            throw ValidationException::withMessages(['expression' => 'Enter a valid cron expression.']);
        }

        if (! in_array($validated['timezone'], timezone_identifiers_list(), true)) {
            throw ValidationException::withMessages(['timezone' => 'Enter a valid timezone identifier.']);
        }

        return $validated;
    }
}
