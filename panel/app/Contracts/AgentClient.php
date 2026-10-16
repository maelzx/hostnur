<?php

namespace App\Contracts;

use App\Agent\Dto\ApplicationLogSnapshot;
use App\Agent\Dto\ApplicationMetricsSnapshot;
use App\Agent\Dto\ApplicationObservation;
use App\Agent\Dto\CancellationReceipt;
use App\Agent\Dto\CapabilitySnapshot;
use App\Agent\Dto\OperationReceipt;
use App\Agent\Dto\OperationRequest;
use App\Agent\Dto\OperationSnapshot;
use App\Agent\Dto\ServerSnapshot;

interface AgentClient
{
    /**
     * Report protocol/mode support and availability. Never a mutation.
     */
    public function capabilities(): CapabilitySnapshot;

    /**
     * Return a bounded read-only server observation.
     */
    public function snapshot(): ServerSnapshot;

    /**
     * Accept an operation intent. Acceptance is not completion.
     */
    public function submit(OperationRequest $request): OperationReceipt;

    /**
     * Read the durable state of a previously accepted execution.
     */
    public function inspect(string $executionId): OperationSnapshot;

    /**
     * Request cancellation of an accepted execution.
     */
    public function cancel(string $executionId): CancellationReceipt;

    /**
     * Register/refresh an application's source and domain in the agent registry
     * before a real deployment. No-op for adapters without a durable registry.
     */
    public function registerApplication(OperationRequest $request): void;

    /**
     * Register/refresh an application schedule (command, cron, timezone) in the
     * agent registry before applying it. No-op without a durable registry.
     */
    public function registerSchedule(OperationRequest $request): void;

    /**
     * Register a database allocation and return the agent-derived identifiers
     * (`database`, `username`). Real adapters return the registry response;
     * others return an empty array.
     *
     * @return array<string, mixed>
     */
    public function registerDatabase(OperationRequest $request): array;

    /**
     * Drop a registered managed database. No-op without a durable registry.
     */
    public function removeDatabase(OperationRequest $request): void;

    /**
     * Rotate a managed database credential. No-op without a durable registry.
     */
    public function rotateDatabase(OperationRequest $request): void;

    /**
     * Register a certificate request (domain hostname + owning application) with
     * the agent. No-op without a durable registry.
     */
    public function registerCertificate(OperationRequest $request): void;

    /**
     * Read a bounded, redacted tail of an application's own log files.
     */
    public function applicationLogs(OperationRequest $request): ApplicationLogSnapshot;

    /**
     * Read per-application storage metrics.
     */
    public function applicationMetrics(OperationRequest $request): ApplicationMetricsSnapshot;

    /**
     * Read the agent-observed application lifecycle and active release.
     */
    public function applicationObservation(OperationRequest $request): ApplicationObservation;
}
