<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;

class FakeAgentExecution extends Model
{
    /**
     * @var list<string>
     */
    protected $fillable = [
        'execution_id',
        'request_id',
        'idempotency_key',
        'operation',
        'resource_kind',
        'resource_id',
        'parameters',
        'scenario',
        'read_only',
    ];

    /**
     * @return array<string, string>
     */
    protected function casts(): array
    {
        return [
            'parameters' => 'array',
            'read_only' => 'boolean',
            'cancel_requested_at' => 'datetime',
        ];
    }
}
