<?php

return [

    /*
    |--------------------------------------------------------------------------
    | Recent Authentication Window
    |--------------------------------------------------------------------------
    |
    | Sensitive administrator changes require the account password to have been
    | confirmed within this many seconds. The same value is mirrored by the
    | "auth.password_timeout" setting so Fortify's password.confirm middleware
    | uses the same window.
    |
    */

    'recent_auth_timeout' => (int) env('HOSTNUR_RECENT_AUTH_TIMEOUT', 300),

];
