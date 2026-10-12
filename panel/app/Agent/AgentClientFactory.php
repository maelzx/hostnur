<?php

namespace App\Agent;

use App\Agent\Exceptions\AgentConfigurationException;
use App\Contracts\AgentClient;
use App\Enums\AgentMode;
use Illuminate\Contracts\Foundation\Application;

final class AgentClientFactory
{
    public function __construct(
        private readonly Application $app,
    ) {}

    public function make(): AgentClient
    {
        $configured = (string) config('hostnur.agent_mode', AgentMode::Unavailable->value);
        $mode = AgentMode::tryFrom($configured);

        if ($mode === null) {
            throw new AgentConfigurationException("Unknown agent mode [{$configured}].");
        }

        return match ($mode) {
            AgentMode::Unavailable => new UnavailableAgentClient,
            AgentMode::Real => new UnavailableAgentClient(
                'The real agent transport is not implemented until phase 11.'
            ),
            AgentMode::Fake => $this->makeFake(),
        };
    }

    /**
     * Fake mode is local/testing only. This is checked both when binding the
     * adapter and again before the fake adapter executes a mutation.
     */
    public function assertFakeModeAllowed(): void
    {
        if (! $this->app->environment('local', 'testing')) {
            throw new AgentConfigurationException(
                'Fake agent mode is not allowed outside the local or testing environment.'
            );
        }
    }

    private function makeFake(): AgentClient
    {
        $this->assertFakeModeAllowed();

        throw new AgentConfigurationException('The fake agent adapter is not implemented yet.');
    }
}
