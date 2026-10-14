<?php

namespace App\Agent;

use App\Agent\Dto\CancellationReceipt;
use App\Agent\Dto\CapabilitySnapshot;
use App\Agent\Dto\OperationReceipt;
use App\Agent\Dto\OperationRequest;
use App\Agent\Dto\OperationSnapshot;
use App\Agent\Dto\ServerSnapshot;
use App\Agent\Exceptions\AgentUnavailableException;
use App\Contracts\AgentClient;
use App\Enums\AgentMode;

final class UnavailableAgentClient implements AgentClient
{
    private const DEFAULT_REASON = 'No agent transport is configured.';

    public function __construct(
        private readonly string $reason = self::DEFAULT_REASON,
    ) {}

    public function capabilities(): CapabilitySnapshot
    {
        return new CapabilitySnapshot(
            protocolVersion: $this->protocolVersion(),
            mode: AgentMode::Unavailable,
            available: false,
            operations: [],
            sampledAt: now()->toIso8601String(),
            reason: $this->reason,
        );
    }

    public function snapshot(): ServerSnapshot
    {
        return new ServerSnapshot(
            protocolVersion: $this->protocolVersion(),
            mode: AgentMode::Unavailable,
            available: false,
            sampledAt: now()->toIso8601String(),
            metrics: [],
            services: [],
            reason: $this->reason,
        );
    }

    public function submit(OperationRequest $request): OperationReceipt
    {
        throw new AgentUnavailableException('The agent is unavailable, so the operation was not submitted.');
    }

    public function inspect(string $executionId): OperationSnapshot
    {
        throw new AgentUnavailableException('The agent is unavailable, so execution state cannot be read.');
    }

    public function cancel(string $executionId): CancellationReceipt
    {
        throw new AgentUnavailableException('The agent is unavailable, so the operation was not cancelled.');
    }

    public function registerApplication(OperationRequest $request): void
    {
        throw new AgentUnavailableException('The agent is unavailable, so the application was not registered.');
    }

    public function registerSchedule(OperationRequest $request): void
    {
        throw new AgentUnavailableException('The agent is unavailable, so the schedule was not registered.');
    }

    public function registerDatabase(OperationRequest $request): array
    {
        throw new AgentUnavailableException('The agent is unavailable, so the database was not registered.');
    }

    public function removeDatabase(OperationRequest $request): void
    {
        throw new AgentUnavailableException('The agent is unavailable, so the database was not removed.');
    }

    public function rotateDatabase(OperationRequest $request): void
    {
        throw new AgentUnavailableException('The agent is unavailable, so the credential was not rotated.');
    }

    private function protocolVersion(): int
    {
        return (int) config('hostnur.protocol_version', 1);
    }
}
