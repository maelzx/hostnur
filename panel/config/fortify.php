<?php

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
    | available. Optional features are added only when their slice implements
    | them: registration, password reset, email verification, profile/password
    | updates, two-factor authentication and passkeys remain disabled.
    |
    */

    'features' => [
        //
    ],

];
