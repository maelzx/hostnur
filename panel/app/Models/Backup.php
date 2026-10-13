<?php

namespace App\Models;

use App\Enums\RequestState;
use Illuminate\Database\Eloquent\Concerns\HasUuids;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

class Backup extends Model
{
    use HasUuids;

    /**
     * @var list<string>
     */
    protected $fillable = [
        'backup_policy_id',
        'application_id',
        'mode',
        'state',
        'verification_status',
        'size_bytes',
        'task_id',
    ];

    /**
     * @return array<string, string>
     */
    protected function casts(): array
    {
        return [
            'state' => RequestState::class,
            'size_bytes' => 'integer',
        ];
    }

    public function policy(): BelongsTo
    {
        return $this->belongsTo(BackupPolicy::class, 'backup_policy_id');
    }

    public function application(): BelongsTo
    {
        return $this->belongsTo(Application::class);
    }

    public function task(): BelongsTo
    {
        return $this->belongsTo(Task::class);
    }

    public function isSimulated(): bool
    {
        return $this->mode === 'fake';
    }

    public function isRecoverable(): bool
    {
        return $this->verification_status === 'verified';
    }
}
