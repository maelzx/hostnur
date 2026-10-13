<?php

namespace App\Services;

use App\Enums\AuditAction;
use App\Enums\AuditOutcome;
use App\Models\AuditEvent;
use App\Models\Task;
use App\Models\User;
use App\Support\Redactor;

class AuditRecorder
{
    /**
     * @var list<string>
     */
    public const METADATA_ALLOW = [
        'operation', 'mode', 'email', 'reason', 'code', 'status', 'execution_id', 'original_task_id',
    ];

    /**
     * @param  array<string, mixed>  $context
     */
    public function record(?User $actor, AuditAction $action, AuditOutcome $outcome, array $context = []): AuditEvent
    {
        return AuditEvent::create([
            'actor_id' => $actor?->getKey(),
            'actor_label' => $actor?->email ?? ($context['actor_label'] ?? null),
            'action' => $action->value,
            'outcome' => $outcome->value,
            'resource_kind' => $context['resource_kind'] ?? null,
            'resource_id' => $context['resource_id'] ?? null,
            'task_id' => $context['task_id'] ?? null,
            'metadata' => Redactor::metadata($context['metadata'] ?? [], self::METADATA_ALLOW),
            'recorded_at' => now(),
        ]);
    }

    /**
     * @param  array<string, mixed>  $metadata
     */
    public function forTask(?User $actor, AuditAction $action, AuditOutcome $outcome, Task $task, array $metadata = []): AuditEvent
    {
        return $this->record($actor, $action, $outcome, [
            'resource_kind' => $task->resource_kind,
            'resource_id' => $task->resource_id,
            'task_id' => $task->getKey(),
            'metadata' => array_merge([
                'operation' => $task->operation,
                'mode' => $task->mode,
                'status' => $task->status->value,
            ], $metadata),
        ]);
    }
}
