<?php

namespace App\Models;

use App\Enums\DomainHttpsPolicy;
use Illuminate\Database\Eloquent\Concerns\HasUuids;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

class ApplicationDomain extends Model
{
    use HasUuids;

    /**
     * @var list<string>
     */
    protected $fillable = [
        'application_id',
        'hostname',
        'https_policy',
        'observed_routing_state',
        'observed_certificate_state',
        'observed_provenance',
    ];

    /**
     * @return array<string, string>
     */
    protected function casts(): array
    {
        return [
            'https_policy' => DomainHttpsPolicy::class,
        ];
    }

    public function application(): BelongsTo
    {
        return $this->belongsTo(Application::class);
    }
}
