<?php

use Laravel\Fortify\Actions\AttemptToAuthenticate;
use Laravel\Fortify\Actions\CanonicalizeUsername;
use Laravel\Fortify\Actions\PrepareAuthenticatedSession;
use Laravel\Fortify\Features;

return [

    /*
    |--------------------------------------------------------------------------
    | Fortify Guard / Password Broker / Username
    |--------------------------------------------------------------------------
    |
    | Hostnur uses Laravel's session "web" guard and the "users" password
    | broker. Usernames are normalized to lowercase to match the one-time
    | administrator bootstrap command.
    |
    */

    'guard' => 'web',

    'passwords' => 'users',

    'username' => 'email',

    'email' => 'email',

    'lowercase_usernames' => true,

    /*
    |--------------------------------------------------------------------------
    | Home / Redirects / Prefix
    |--------------------------------------------------------------------------
    |
    | Authorized administrators land on the restricted panel area. Fortify owns
    | the authentication POST endpoints; Hostnur renders its own login screen
    | and disables Fortify's built-in view routes, so only the routes the login
    | slice actually implements are registered.
    |
    */

    'home' => '/panel',

    'redirects' => [
        'login' => '/panel',
        'logout' => '/login',
    ],

    'prefix' => '',

    'domain' => null,

    'middleware' => ['web'],

    /*
    |--------------------------------------------------------------------------
    | Login Pipeline
    |--------------------------------------------------------------------------
    |
    | Hostnur authenticates through an administrator-only callback and its own
    | named rate limiter. Two-factor login enforcement is intentionally omitted
    | from this pipeline until P2.4 adds the challenge screen; P2.4 will add
    | RedirectsIfTwoFactorAuthenticatable here. Until then an enrolled
    | administrator still signs in with a password only.
    |
    */

    'pipelines' => [
        'login' => [
            CanonicalizeUsername::class,
            AttemptToAuthenticate::class,
            PrepareAuthenticatedSession::class,
        ],
    ],

    /*
    |--------------------------------------------------------------------------
    | Rate Limiting
    |--------------------------------------------------------------------------
    |
    | The "login" limiter is defined in App\Providers\FortifyServiceProvider and
    | enforces five attempts per minute per normalized email + IP plus thirty
    | attempts per minute per IP. Two-factor and passkey limiters are unset
    | because those features are not enabled yet.
    |
    */

    'limiters' => [
        'login' => 'login',
        'two-factor' => null,
        'passkeys' => null,
    ],

    /*
    |--------------------------------------------------------------------------
    | Register View Routes
    |--------------------------------------------------------------------------
    |
    | Disabled so Fortify does not register login/two-factor/password-confirm
    | view routes that Hostnur has not implemented. Hostnur owns the GET login
    | screen and Fortify owns the POST login/logout endpoints.
    |
    */

    'views' => false,

    /*
    |--------------------------------------------------------------------------
    | Passkeys
    |--------------------------------------------------------------------------
    |
    | Required keys for Fortify's passkey integration configuration, even though
    | the passkeys feature is not enabled in this slice.
    |
    */

    'passkeys' => [
        'relying_party_id' => parse_url(config('app.url'), PHP_URL_HOST),
        'allowed_origins' => [config('app.url')],
        'timeout' => 60000,
    ],

    /*
    |--------------------------------------------------------------------------
    | Features
    |--------------------------------------------------------------------------
    |
    | Login and logout are part of Fortify's base routes and are always
    | available. Two-factor authentication enrollment is enabled with required
    | confirmation and a single-step TOTP window; recent-password confirmation
    | for sensitive changes arrives in P2.5, and login enforcement in P2.4.
    | Registration, password reset, email verification, profile/password
    | updates and passkeys remain disabled.
    |
    */

    'features' => [
        Features::twoFactorAuthentication([
            'confirm' => true,
            'confirmPassword' => false,
            'window' => 1,
        ]),
    ],

];
