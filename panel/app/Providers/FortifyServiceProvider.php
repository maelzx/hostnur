<?php

namespace App\Providers;

use App\Models\User;
use Illuminate\Auth\Middleware\RedirectIfAuthenticated;
use Illuminate\Cache\RateLimiting\Limit;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Event;
use Illuminate\Support\Facades\Hash;
use Illuminate\Support\Facades\RateLimiter;
use Illuminate\Support\ServiceProvider;
use Illuminate\Support\Str;
use Laravel\Fortify\Events\ValidTwoFactorAuthenticationCodeProvided;
use Laravel\Fortify\Fortify;

class FortifyServiceProvider extends ServiceProvider
{
    public function register(): void
    {
        //
    }

    public function boot(): void
    {
        // Only an existing administrator with valid credentials may authenticate.
        // Unknown emails, ordinary accounts and wrong passwords all fail with the
        // same generic message because the callback returns null.
        Fortify::authenticateUsing(function (Request $request): ?User {
            $email = Str::lower(trim((string) $request->input(Fortify::username())));

            $user = User::query()->where('email', $email)->first();

            if ($user === null || $user->is_admin !== true) {
                return null;
            }

            $password = $request->input('password');

            if (! is_string($password) || ! Hash::check($password, $user->password)) {
                return null;
            }

            return $user;
        });

        // Record the generation-bound MFA proof only after a valid TOTP or
        // recovery code. It is bound to the user id and current mfa_version so
        // password/MFA rotation invalidates older sessions.
        Event::listen(ValidTwoFactorAuthenticationCodeProvided::class, function (ValidTwoFactorAuthenticationCodeProvided $event): void {
            if (request()->hasSession()) {
                request()->session()->put('mfa_verified', [
                    'id' => $event->user->getKey(),
                    'version' => $event->user->mfa_version,
                ]);
            }
        });

        RateLimiter::for('login', function (Request $request): array {
            $email = Str::lower(trim((string) $request->input(Fortify::username())));

            return [
                Limit::perMinute(5)->by(Str::transliterate($email.'|'.$request->ip())),
                Limit::perMinute(30)->by($request->ip()),
            ];
        });

        RateLimiter::for('two-factor', function (Request $request): Limit {
            $challengedUser = (string) $request->session()->get('login.id');

            return Limit::perMinute(5)->by($challengedUser.'|'.$request->ip());
        });

        RedirectIfAuthenticated::redirectUsing(
            fn (): string => route('panel.dashboard'),
        );
    }
}
