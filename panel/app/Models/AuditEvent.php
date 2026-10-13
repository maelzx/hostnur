<?php

namespace App\Models;

use App\Enums\AuditAction;
use App\Enums\AuditOutcome;
use Illuminate\Database\Eloquent\Concerns\HasUuids;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

class AuditEvent extends Model
{
    use HasUuids;

    public $timestamps = false;

    /**
     * @var list<string>
     */
    protected $fillable = [
        'actor_id',
        'actor_label',
        'action',
        'outcome',
        'resource_kind',
        'resource_id',
        'task_id',
        'metadata',
        'recorded_at',
    ];

    /**
     * @return array<string, string>
     */
    protected function casts(): array
    {
        return [
            'action' => AuditAction::class,
            'outcome' => AuditOutcome::class,
            'metadata' => 'array',
            'recorded_at' => 'datetime',
        ];
    }

    public function actor(): BelongsTo
    {
        return $this->belongsTo(User::class, 'actor_id');
    }
}
