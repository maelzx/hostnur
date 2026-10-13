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

    /*
    |--------------------------------------------------------------------------
    | Agent Mode
    |--------------------------------------------------------------------------
    |
    | "unavailable" is the safe default and is the only mode allowed in
    | production until a real transport exists (phase 11). "fake" may be selected
    | only in local/testing environments. "real" reports unavailable for now.
    |
    */

    'agent_mode' => env('HOSTNUR_AGENT_MODE', 'unavailable'),

    /*
    |--------------------------------------------------------------------------
    | Agent Contract
    |--------------------------------------------------------------------------
    |
    | Supported protocol version, the versioned contract directory and the
    | initial bounded sizes. Limits are enforced by request validation and DTO
    | parsing, not scattered through UI components.
    |
    */

    'protocol_version' => 1,

    'contracts_path' => env('HOSTNUR_CONTRACTS_PATH', dirname(base_path()).'/contracts/v1'),

    'limits' => [
        'max_message_bytes' => 1_048_576,
        'max_parameters' => 64,
        'max_events' => 100,
        'max_observations' => 100,
    ],

    /*
    |--------------------------------------------------------------------------
    | Fake Agent Simulation
    |--------------------------------------------------------------------------
    |
    | The deterministic fake adapter derives progress from the record's creation
    | time with a controlled clock (no sleeps or workers). `scenario` selects the
    | outcome for new executions: success, failure or timeout.
    |
    */

    'fake' => [
        'scenario' => env('HOSTNUR_FAKE_SCENARIO', 'success'),
        'duration_seconds' => 2,
        'timeout_seconds' => 3,
    ],

];
