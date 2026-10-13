<?php

namespace App\Listeners;

use App\Enums\AuditAction;
use App\Enums\AuditOutcome;
use App\Services\AuditRecorder;
use Illuminate\Auth\Events\Failed;
use Illuminate\Auth\Events\Lockout;
use Illuminate\Auth\Events\Login;
use Illuminate\Auth\Events\Logout;
use Illuminate\Events\Dispatcher;
use Laravel\Fortify\Events\RecoveryCodeReplaced;
use Laravel\Fortify\Events\RecoveryCodesGenerated;
use Laravel\Fortify\Events\TwoFactorAuthenticationChallenged;
use Laravel\Fortify\Events\TwoFactorAuthenticationConfirmed;
use Laravel\Fortify\Events\TwoFactorAuthenticationDisabled;
use Laravel\Fortify\Events\TwoFactorAuthenticationEnabled;
use Laravel\Fortify\Events\ValidTwoFactorAuthenticationCodeProvided;

/**
 * Records authentication and MFA events for the auditor without ever storing
 * passwords, codes, recovery strings or provisioning secrets.
 */
class AuthenticationEventSubscriber
{
    public function __construct(
        private readonly AuditRecorder $audit,
    ) {}

    public function subscribe(Dispatcher $events): void
    {
        $events->listen(Login::class, fn (Login $event) => $this->audit->record($event->user, AuditAction::AuthLogin, AuditOutcome::Success));
        $events->listen(Logout::class, fn (Logout $event) => $this->audit->record($event->user, AuditAction::AuthLogout, AuditOutcome::Success));

        $events->listen(Failed::class, fn (Failed $event) => $this->audit->record(
            null,
            AuditAction::AuthLoginFailed,
            AuditOutcome::Failure,
            ['metadata' => ['email' => $this->credential($event)]],
        ));

        $events->listen(Lockout::class, fn (Lockout $event) => $this->audit->record(
            null,
            AuditAction::AuthLockout,
            AuditOutcome::Denied,
            ['metadata' => ['email' => (string) $event->request->input('email')]],
        ));

        $events->listen(TwoFactorAuthenticationEnabled::class, fn (TwoFactorAuthenticationEnabled $event) => $this->audit->record($event->user, AuditAction::AuthMfaEnrolled, AuditOutcome::Success));
        $events->listen(TwoFactorAuthenticationConfirmed::class, fn (TwoFactorAuthenticationConfirmed $event) => $this->audit->record($event->user, AuditAction::AuthMfaConfirmed, AuditOutcome::Success));
        $events->listen(TwoFactorAuthenticationDisabled::class, fn (TwoFactorAuthenticationDisabled $event) => $this->audit->record($event->user, AuditAction::AuthMfaDisabled, AuditOutcome::Success));
        $events->listen(TwoFactorAuthenticationChallenged::class, fn (TwoFactorAuthenticationChallenged $event) => $this->audit->record($event->user, AuditAction::AuthMfaChallenged, AuditOutcome::Requested));
        $events->listen(ValidTwoFactorAuthenticationCodeProvided::class, fn (ValidTwoFactorAuthenticationCodeProvided $event) => $this->audit->record($event->user, AuditAction::AuthMfaVerified, AuditOutcome::Success));
        $events->listen(RecoveryCodeReplaced::class, fn (RecoveryCodeReplaced $event) => $this->audit->record($event->user, AuditAction::AuthRecoveryCodeUsed, AuditOutcome::Success));
        $events->listen(RecoveryCodesGenerated::class, fn (RecoveryCodesGenerated $event) => $this->audit->record($event->user, AuditAction::AuthRecoveryCodesRegenerated, AuditOutcome::Success));
    }

    private function credential(Failed $event): ?string
    {
        $email = $event->credentials['email'] ?? null;

        return is_string($email) ? $email : null;
    }
}
