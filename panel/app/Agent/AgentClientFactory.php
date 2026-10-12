<?php

namespace App\Agent;

use App\Agent\Exceptions\AgentConfigurationException;
use App\Contracts\AgentClient;
use App\Enums\AgentMode;

final class AgentClientFactory
{
    public function __construct(
        private readonly AgentModeGuard $guard,
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

    private function makeFake(): AgentClient
    {
        $this->guard->assertFakeModeAllowed();

        return new FakeAgentClient($this->guard);
    }
}
