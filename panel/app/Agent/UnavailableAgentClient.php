<?php

namespace App\Agent;

use App\Agent\Dto\ApplicationLogSnapshot;
use App\Agent\Dto\ApplicationMetricsSnapshot;
use App\Agent\Dto\ApplicationObservation;
use App\Agent\Dto\CancellationReceipt;
use App\Agent\Dto\CapabilitySnapshot;
use App\Agent\Dto\OperationReceipt;
use App\Agent\Dto\OperationRequest;
use App\Agent\Dto\OperationSnapshot;
use App\Agent\Dto\ServerSnapshot;
use App\Agent\Dto\ServerUpdatesSnapshot;
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

    public function removeSchedule(OperationRequest $request): void
    {
        throw new AgentUnavailableException('The agent is unavailable, so the schedule was not removed.');
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

    public function registerCertificate(OperationRequest $request): void
    {
        throw new AgentUnavailableException('The agent is unavailable, so the certificate was not registered.');
    }

    public function applicationLogs(OperationRequest $request): ApplicationLogSnapshot
    {
        throw new AgentUnavailableException('The agent is unavailable, so application logs cannot be read.');
    }

    public function applicationMetrics(OperationRequest $request): ApplicationMetricsSnapshot
    {
        throw new AgentUnavailableException('The agent is unavailable, so application metrics cannot be read.');
    }

    public function applicationObservation(OperationRequest $request): ApplicationObservation
    {
        throw new AgentUnavailableException('The agent is unavailable, so the observed state cannot be read.');
    }

    public function serverUpdates(OperationRequest $request): ServerUpdatesSnapshot
    {
        throw new AgentUnavailableException('The agent is unavailable, so package updates cannot be read.');
    }

    public function registerPackage(OperationRequest $request): void
    {
        throw new AgentUnavailableException('The agent is unavailable, so the package was not registered.');
    }

    public function removePackage(OperationRequest $request): void
    {
        throw new AgentUnavailableException('The agent is unavailable, so the package was not removed.');
    }

    public function registerBackup(OperationRequest $request): void
    {
        throw new AgentUnavailableException('The agent is unavailable, so the backup was not registered.');
    }

    private function protocolVersion(): int
    {
        return (int) config('hostnur.protocol_version', 1);
    }
}
