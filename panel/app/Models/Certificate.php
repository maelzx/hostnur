<?php

namespace App\Models;

use App\Enums\CertificateSource;
use App\Enums\RequestState;
use Illuminate\Database\Eloquent\Concerns\HasUuids;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

class Certificate extends Model
{
    use HasUuids;

    /**
     * @var list<string>
     */
    protected $fillable = [
        'application_domain_id',
        'hostname',
        'source',
        'provider',
        'certificate_pem',
        'private_key',
        'passphrase',
        'issued_at',
        'expires_at',
        'state',
        'task_id',
        'observed_state',
        'observed_provenance',
    ];

    /**
     * @var list<string>
     */
    protected $hidden = [
        'private_key',
        'passphrase',
    ];

    /**
     * @return array<string, string>
     */
    protected function casts(): array
    {
        return [
            'source' => CertificateSource::class,
            'state' => RequestState::class,
            'private_key' => 'encrypted',
            'passphrase' => 'encrypted',
            'issued_at' => 'datetime',
            'expires_at' => 'datetime',
        ];
    }

    public function domain(): BelongsTo
    {
        return $this->belongsTo(ApplicationDomain::class, 'application_domain_id');
    }

    public function task(): BelongsTo
    {
        return $this->belongsTo(Task::class);
    }

    public function isExpired(): bool
    {
        return $this->expires_at !== null && $this->expires_at->isPast();
    }
}
