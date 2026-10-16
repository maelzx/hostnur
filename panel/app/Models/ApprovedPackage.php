<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Concerns\HasUuids;
use Illuminate\Database\Eloquent\Model;

/**
 * An operator-approved package name beyond the built-in catalog. Kept in sync
 * with the agent registry.
 */
class ApprovedPackage extends Model
{
    use HasUuids;

    /**
     * @var list<string>
     */
    protected $fillable = [
        'name',
    ];
}
