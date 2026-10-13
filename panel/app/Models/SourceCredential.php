<?php

namespace App\Models;

use App\Enums\SourceCredentialType;
use Illuminate\Database\Eloquent\Concerns\HasUuids;
use Illuminate\Database\Eloquent\Model;

class SourceCredential extends Model
{
    use HasUuids;

    /**
     * @var list<string>
     */
    protected $fillable = [
        'label',
        'type',
        'secret',
        'public_key',
        'host_key',
        'version',
        'rotated_at',
    ];

    /**
     * @var list<string>
     */
    protected $hidden = [
        'secret',
    ];

    /**
     * @return array<string, string>
     */
    protected function casts(): array
    {
        return [
            'type' => SourceCredentialType::class,
            'secret' => 'encrypted',
            'version' => 'integer',
            'rotated_at' => 'datetime',
        ];
    }
}
