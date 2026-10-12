<?php

namespace App\Livewire\Panel;

use App\Models\User;
use App\Services\RecentAuthentication;
use Closure;
use Illuminate\Contracts\View\View;
use Illuminate\Support\Facades\Auth;
use Illuminate\Validation\Rules\Password;
use Laravel\Fortify\Actions\DisableTwoFactorAuthentication;
use Laravel\Fortify\Actions\GenerateNewRecoveryCodes;
use Livewire\Attributes\Layout;
use Livewire\Component;

#[Layout('layouts.app')]
class SecuritySettings extends Component
{
    public string $password = '';

    public string $password_confirmation = '';

    public function changePassword(): void
    {
        $user = $this->administrator();
        $this->requireRecentAuthentication($user);

        $this->validate([
            'password' => [
                'bail', 'required', 'string', Password::min(12), 'confirmed',
                function (string $attribute, mixed $value, Closure $fail): void {
                    if (strlen((string) $value) > 72) {
                        $fail('The password must not exceed 72 bytes.');
                    }

                    if (str_contains((string) $value, "\0")) {
                        $fail('The password must not contain null bytes.');
                    }
                },
            ],
        ]);

        $user->forceFill(['password' => $this->password])->save();

        $this->reset('password', 'password_confirmation');

        $this->revokeSessions($user, 'Your password was changed. Sign in again.');
    }

    public function disableTwoFactor(): void
    {
        $user = $this->administrator();
        $this->requireRecentAuthentication($user);

        app(DisableTwoFactorAuthentication::class)($user);
        $user->refresh();

        $this->revokeSessions($user, 'Multi-factor authentication was disabled. Sign in again.');
    }

    public function regenerateRecoveryCodes(): void
    {
        $user = $this->administrator();
        $this->requireRecentAuthentication($user);

        app(GenerateNewRecoveryCodes::class)($user);
        $user->refresh();

        session()->flash('recovery_codes', $user->recoveryCodes());

        $this->redirect(route('panel.security'));
    }

    public function render(): View
    {
        $user = $this->administrator();

        return view('livewire.panel.security-settings', [
            'user' => $user,
            'recoveryCodes' => session()->pull('recovery_codes', []),
        ]);
    }

    private function requireRecentAuthentication(User $user): void
    {
        if (! app(RecentAuthentication::class)->isConfirmed($user)) {
            abort(423, 'Password confirmation required.');
        }
    }

    private function revokeSessions(User $user, string $status): void
    {
        $user->forceFill(['mfa_version' => $user->mfa_version + 1])->save();

        app(RecentAuthentication::class)->forget();

        Auth::guard('web')->logout();
        session()->invalidate();
        session()->regenerateToken();
        session()->flash('status', $status);

        $this->redirect(route('login'));
    }

    private function administrator(): User
    {
        $user = auth()->user();

        if (! $user instanceof User || $user->is_admin !== true) {
            abort(403);
        }

        return $user;
    }
}
