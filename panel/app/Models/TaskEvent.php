<?php

namespace App\Models;

use App\Enums\TaskEventLevel;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

class TaskEvent extends Model
{
    public $timestamps = false;

    /**
     * @var list<string>
     */
    protected $fillable = [
        'task_id',
        'sequence',
        'type',
        'level',
        'message',
        'metadata',
        'recorded_at',
    ];

    /**
     * @return array<string, string>
     */
    protected function casts(): array
    {
        return [
            'level' => TaskEventLevel::class,
            'metadata' => 'array',
            'recorded_at' => 'datetime',
            'sequence' => 'integer',
        ];
    }

    public function task(): BelongsTo
    {
        return $this->belongsTo(Task::class);
    }
}
