<?php

namespace App\Models;

use App\Enums\ApplicationStatus;
use App\Enums\SourceStatus;
use Illuminate\Database\Eloquent\Concerns\HasUuids;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Relations\HasMany;
use Illuminate\Database\Eloquent\Relations\HasOne;

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

    public function scheduledJobs(): HasMany
    {
        return $this->hasMany(ScheduledJob::class)->orderBy('name');
    }

    public function databaseRequests(): HasMany
    {
        return $this->hasMany(DatabaseRequest::class)->latest('created_at');
    }

    public function backupPolicies(): HasMany
    {
        return $this->hasMany(BackupPolicy::class)->latest('created_at');
    }

    public function backups(): HasMany
    {
        return $this->hasMany(Backup::class)->latest('created_at');
    }

    public function deployments(): HasMany
    {
        return $this->hasMany(Deployment::class);
    }

    public function latestDeployment(): HasOne
    {
        return $this->hasOne(Deployment::class)
            ->orderByDesc('requested_at')
            ->orderByDesc('created_at');
    }

    public function sourceCredential(): BelongsTo
    {
        return $this->belongsTo(SourceCredential::class);
    }

    public function isArchived(): bool
    {
        return $this->archived_at !== null;
    }

    public function status(): ApplicationStatus
    {
        if ($this->isArchived()) {
            return ApplicationStatus::Archived;
        }

        return $this->latestDeployment === null
            ? ApplicationStatus::NotDeployed
            : ApplicationStatus::fromDeploymentState($this->latestDeployment->state);
    }
}
