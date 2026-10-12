<?php

namespace App\Agent;

use App\Agent\Exceptions\AgentConfigurationException;
use Illuminate\Contracts\Foundation\Application;

final class AgentModeGuard
{
    public function __construct(
        private readonly Application $app,
    ) {}

    /**
     * Fake mode is local/testing only. It is checked both when binding the
     * adapter and again before the fake adapter executes anything.
     */
    public function assertFakeModeAllowed(): void
    {
        if (! $this->app->environment('local', 'testing')) {
            throw new AgentConfigurationException(
                'Fake agent mode is not allowed outside the local or testing environment.'
            );
        }
    }
}
