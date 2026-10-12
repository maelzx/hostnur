<?php

namespace App\Agent;

use App\Agent\Dto\CancellationReceipt;
use App\Agent\Dto\CapabilitySnapshot;
use App\Agent\Dto\OperationReceipt;
use App\Agent\Dto\OperationRequest;
use App\Agent\Dto\OperationSnapshot;
use App\Agent\Dto\ServerSnapshot;
use App\Contracts\AgentClient;
use RuntimeException;

/**
 * Deterministic, read-only simulated agent. It never touches the network, the
 * filesystem outside the checked-in contract fixtures, or any host daemon.
 * Durable submit/inspect/cancel simulation is added in P3.4.
 */
final class FakeAgentClient implements AgentClient
{
    public function __construct(
        private readonly AgentModeGuard $guard,
    ) {}

    public function capabilities(): CapabilitySnapshot
    {
        $this->guard->assertFakeModeAllowed();

        $fixture = $this->fixture('capability');
        $fixture['sampled_at'] = now()->toIso8601String();

        return CapabilitySnapshot::fromArray($fixture);
    }

    public function snapshot(): ServerSnapshot
    {
        $this->guard->assertFakeModeAllowed();

        $fixture = $this->fixture('server-snapshot');
        $fixture['sampled_at'] = now()->toIso8601String();

        return ServerSnapshot::fromArray($fixture);
    }

    public function submit(OperationRequest $request): OperationReceipt
    {
        throw AgentContractException::unsupportedOperation(
            'The fake agent does not implement durable operation execution yet.'
        );
    }

    public function inspect(string $executionId): OperationSnapshot
    {
        throw AgentContractException::unsupportedOperation(
            'The fake agent does not implement durable operation execution yet.'
        );
    }

    public function cancel(string $executionId): CancellationReceipt
    {
        throw AgentContractException::unsupportedOperation(
            'The fake agent does not implement durable operation execution yet.'
        );
    }

    /**
     * @return array<string, mixed>
     */
    private function fixture(string $name): array
    {
        $path = rtrim((string) config('hostnur.contracts_path'), '/')."/fixtures/fake/{$name}.json";

        if (! is_file($path)) {
            throw new RuntimeException("Fake agent fixture was not found at {$path}.");
        }

        $decoded = json_decode((string) file_get_contents($path), true, flags: JSON_THROW_ON_ERROR);

        if (! is_array($decoded)) {
            throw new RuntimeException("Fake agent fixture {$name} is malformed.");
        }

        return $decoded;
    }
}
