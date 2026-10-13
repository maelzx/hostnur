<?php

namespace App\Models;

use App\Enums\BackupScope;
use Illuminate\Database\Eloquent\Concerns\HasUuids;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Relations\HasMany;

class BackupPolicy extends Model
{
    use HasUuids;

    /**
     * @var list<string>
     */
    protected $fillable = [
        'application_id',
        'scope',
        'expression',
        'timezone',
        'retention_days',
        'destination_credential_id',
        'enabled',
    ];

    /**
     * @return array<string, string>
     */
    protected function casts(): array
    {
        return [
            'scope' => BackupScope::class,
            'retention_days' => 'integer',
            'enabled' => 'boolean',
        ];
    }

    public function application(): BelongsTo
    {
        return $this->belongsTo(Application::class);
    }

    public function destinationCredential(): BelongsTo
    {
        return $this->belongsTo(SourceCredential::class, 'destination_credential_id');
    }

    public function backups(): HasMany
    {
        return $this->hasMany(Backup::class);
    }
}
