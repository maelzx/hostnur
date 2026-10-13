<?php

namespace App\Models;

use App\Enums\DeploymentState;
use Illuminate\Database\Eloquent\Concerns\HasUuids;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

class Deployment extends Model
{
    use HasUuids;

    /**
     * @var list<string>
     */
    protected $fillable = [
        'application_id',
        'revision_id',
        'revision_number',
        'task_id',
        'operation',
        'git_ref',
        'resolved_commit',
        'runtime',
        'mode',
        'state',
        'run_migrations',
        'migration_acknowledged_at',
        'release_reference',
        'previous_release_reference',
        'health_summary',
        'error_code',
        'error_message',
        'idempotency_key',
        'requested_at',
        'started_at',
        'finished_at',
        'created_by',
    ];

    /**
     * @return array<string, string>
     */
    protected function casts(): array
    {
        return [
            'state' => DeploymentState::class,
            'run_migrations' => 'boolean',
            'health_summary' => 'array',
            'revision_number' => 'integer',
            'migration_acknowledged_at' => 'datetime',
            'requested_at' => 'datetime',
            'started_at' => 'datetime',
            'finished_at' => 'datetime',
        ];
    }

    public function application(): BelongsTo
    {
        return $this->belongsTo(Application::class);
    }

    public function task(): BelongsTo
    {
        return $this->belongsTo(Task::class);
    }

    public function revision(): BelongsTo
    {
        return $this->belongsTo(ApplicationRevision::class, 'revision_id');
    }

    public function isSimulated(): bool
    {
        return $this->mode === 'fake';
    }
}
