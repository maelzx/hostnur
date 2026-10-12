<?php

namespace App\Contracts;

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
}
