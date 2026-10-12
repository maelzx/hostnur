<?php

namespace App\Services;

use App\Models\User;

class RecentAuthentication
{
    /**
     * Record server-side evidence that the user confirmed their password now.
     *
     * The session keys are also read by Fortify/Laravel's password.confirm
     * middleware; the extra version key binds confirmation to the MFA
     * generation so a rotated password/secret invalidates it.
     */
    public function confirm(User $user): void
    {
        session()->put([
            'auth.password_confirmed_at' => now()->unix(),
            'auth.password_confirmed_mfa_version' => $user->mfa_version,
        ]);
    }

    /**
     * Determine if the current session has an unexpired, generation-bound
     * password confirmation.
     */
    public function isConfirmed(User $user): bool
    {
        $confirmedAt = session('auth.password_confirmed_at');
        $confirmedVersion = session('auth.password_confirmed_mfa_version');

        if (! is_numeric($confirmedAt)) {
            return false;
        }

        $timeout = (int) config('hostnur.recent_auth_timeout', 300);

        if (now()->unix() - (int) $confirmedAt > $timeout) {
            return false;
        }

        return (string) $confirmedVersion === (string) $user->mfa_version;
    }

    public function forget(): void
    {
        session()->forget([
            'auth.password_confirmed_at',
            'auth.password_confirmed_mfa_version',
        ]);
    }
}
