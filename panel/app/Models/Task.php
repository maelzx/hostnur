<?php

namespace App\Models;

use App\Enums\TaskEventLevel;
use App\Enums\TaskStatus;
use App\Exceptions\InvalidTaskTransitionException;
use App\Support\Redactor;
use Illuminate\Database\Eloquent\Concerns\HasUuids;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Relations\HasMany;

class Task extends Model
{
    use HasUuids;

    /**
     * @var list<string>
     */
    public const EVENT_METADATA_ALLOW = [
        'status', 'code', 'retryable', 'mode', 'execution_id', 'original_task_id', 'attempt',
    ];

    /**
     * @var list<string>
     */
    protected $fillable = [
        'actor_id',
        'actor_label',
        'operation',
        'resource_kind',
        'resource_id',
        'config_revision',
        'mode',
        'status',
        'idempotency_key',
        'parameters',
        'result',
        'payload_fingerprint',
        'execution_id',
        'requested_at',
        'queued_at',
        'started_at',
        'finished_at',
        'deadline_at',
        'cancel_requested_at',
        'approved_by',
        'approved_fingerprint',
        'approved_at',
        'approval_expires_at',
        'approved_mfa_version',
        'denied_reason_code',
        'error_code',
        'error_message',
    ];

    /**
     * @return array<string, string>
     */
    protected function casts(): array
    {
        return [
            'status' => TaskStatus::class,
            'parameters' => 'array',
            'result' => 'array',
            'requested_at' => 'datetime',
            'queued_at' => 'datetime',
            'started_at' => 'datetime',
            'finished_at' => 'datetime',
            'deadline_at' => 'datetime',
            'cancel_requested_at' => 'datetime',
            'lease_expires_at' => 'datetime',
            'approved_at' => 'datetime',
            'approval_expires_at' => 'datetime',
            'approved_mfa_version' => 'integer',
        ];
    }

    public function actor(): BelongsTo
    {
        return $this->belongsTo(User::class, 'actor_id');
    }

    public function approver(): BelongsTo
    {
        return $this->belongsTo(User::class, 'approved_by');
    }

    public function events(): HasMany
    {
        return $this->hasMany(TaskEvent::class)->orderBy('sequence');
    }

    public function isTerminal(): bool
    {
        return $this->status->isTerminal();
    }

    public function transitionTo(TaskStatus $status): self
    {
        if (! $this->status->canTransitionTo($status)) {
            throw InvalidTaskTransitionException::between($this->status, $status);
        }

        $this->status = $status;

        return $this;
    }

    /**
     * @param  array<string, mixed>  $metadata
     */
    public function recordEvent(string $type, TaskEventLevel $level, string $message, array $metadata = []): TaskEvent
    {
        $sequence = ((int) $this->events()->max('sequence')) + 1;

        return $this->events()->create([
            'sequence' => $sequence,
            'type' => $type,
            'level' => $level->value,
            'message' => Redactor::message($message),
            'metadata' => Redactor::metadata($metadata, self::EVENT_METADATA_ALLOW),
            'recorded_at' => now(),
        ]);
    }
}
