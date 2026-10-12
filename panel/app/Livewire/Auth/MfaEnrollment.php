<?php

namespace App\Livewire\Auth;

use App\Models\User;
use Illuminate\Contracts\View\View;
use Illuminate\Validation\ValidationException;
use Laravel\Fortify\Actions\ConfirmTwoFactorAuthentication;
use Laravel\Fortify\Actions\EnableTwoFactorAuthentication;
use Laravel\Fortify\Fortify;
use Livewire\Attributes\Layout;
use Livewire\Component;

#[Layout('layouts.app')]
class MfaEnrollment extends Component
{
    public string $code = '';

    public function enable(): void
    {
        $user = $this->administrator();

        app(EnableTwoFactorAuthentication::class)($user);

        $user->refresh();
    }

    public function confirm(): void
    {
        $user = $this->administrator();

        $this->validate([
            'code' => ['required', 'string', 'digits:6'],
        ]);

        try {
            app(ConfirmTwoFactorAuthentication::class)($user, $this->code);
        } catch (ValidationException $exception) {
            $this->addError(
                'code',
                $exception->validator->errors()->first('code')
                    ?? 'The provided two factor authentication code was invalid.'
            );

            return;
        }

        // Confirming the code proves possession, so treat this session as verified.
        session()->put('mfa_verified', [
            'id' => $user->getKey(),
            'version' => $user->mfa_version,
        ]);

        $this->code = '';
        $user->refresh();
    }

    public function render(): View
    {
        $user = $this->administrator();
        $pending = $user->two_factor_secret !== null && $user->two_factor_confirmed_at === null;

        return view('livewire.auth.mfa-enrollment', [
            'user' => $user,
            'pending' => $pending,
            'qrSvg' => $pending ? $user->twoFactorQrCodeSvg() : null,
            'secret' => $pending
                ? Fortify::currentEncrypter()->decrypt($user->two_factor_secret)
                : null,
            'recoveryCodes' => $pending ? $user->recoveryCodes() : [],
        ]);
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
