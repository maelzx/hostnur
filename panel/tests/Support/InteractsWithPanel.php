<?php

namespace Tests\Support;

use App\Models\User;

trait InteractsWithPanel
{
    /**
     * @param  array<string, mixed>  $attributes
     */
    protected function confirmedAdmin(array $attributes = []): User
    {
        return User::factory()->create(array_merge([
            'email' => 'admin@example.com',
            'is_admin' => true,
            'two_factor_secret' => 'encrypted',
            'two_factor_confirmed_at' => now(),
            'mfa_version' => 0,
        ], $attributes));
    }

    /**
     * @return array<string, mixed>
     */
    protected function mfaProof(User $admin): array
    {
        return ['mfa_verified' => ['id' => $admin->getKey(), 'version' => $admin->mfa_version]];
    }

    /**
     * @return array<string, mixed>
     */
    protected function recentAuth(User $admin): array
    {
        return [
            'auth.password_confirmed_at' => now()->unix(),
            'auth.password_confirmed_mfa_version' => $admin->mfa_version,
        ];
    }
}
