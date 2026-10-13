<?php

namespace App\Models;

use App\Enums\SourceStatus;
use Illuminate\Database\Eloquent\Concerns\HasUuids;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Relations\HasMany;

class Application extends Model
{
    use HasUuids;

    /**
     * @var list<string>
     */
    protected $fillable = [
        'name',
        'slug',
        'provider',
        'runtime',
        'runtime_version',
        'repository_url',
        'git_ref',
        'source_credential_id',
        'source_status',
        'current_revision',
        'observed_state',
        'observed_mode',
        'observed_at',
        'archived_at',
    ];

    /**
     * @return array<string, string>
     */
    protected function casts(): array
    {
        return [
            'source_status' => SourceStatus::class,
            'current_revision' => 'integer',
            'observed_at' => 'datetime',
            'archived_at' => 'datetime',
        ];
    }

    public function revisions(): HasMany
    {
        return $this->hasMany(ApplicationRevision::class)->orderByDesc('revision');
    }

    public function secrets(): HasMany
    {
        return $this->hasMany(ApplicationSecret::class)->orderBy('name');
    }

    public function domains(): HasMany
    {
        return $this->hasMany(ApplicationDomain::class)->orderBy('hostname');
    }

    public function sourceCredential(): BelongsTo
    {
        return $this->belongsTo(SourceCredential::class);
    }

    public function isArchived(): bool
    {
        return $this->archived_at !== null;
    }
}
