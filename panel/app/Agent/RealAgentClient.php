<?php

namespace App\Agent;

use App\Agent\Dto\AgentError;
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
use App\Agent\Exceptions\AgentRemoteException;
use App\Agent\Transport\AgentTransport;
use App\Contracts\AgentClient;
use Illuminate\Support\Str;

/**
 * Agent client backed by the real local transport.
 *
 * Control methods use the additive `agent.*` operations; the panel never falls
 * back to fake or silently succeeds if the transport is unavailable.
 */
final class RealAgentClient implements AgentClient
{
    public function __construct(
        private readonly AgentTransport $transport,
    ) {}

    public function capabilities(): CapabilitySnapshot
    {
        return CapabilitySnapshot::fromArray($this->call('agent.capabilities'));
    }

    public function snapshot(): ServerSnapshot
    {
        return ServerSnapshot::fromArray($this->call('server.snapshot'));
    }

    public function submit(OperationRequest $request): OperationReceipt
    {
        return OperationReceipt::fromArray($this->send($request->toArray()));
    }

    public function inspect(string $executionId): OperationSnapshot
    {
        return OperationSnapshot::fromArray(
            $this->call('agent.inspect', ['execution_id' => $executionId]),
        );
    }

    public function cancel(string $executionId): CancellationReceipt
    {
        return CancellationReceipt::fromArray(
            $this->call('agent.cancel', ['execution_id' => $executionId]),
        );
    }

    public function registerApplication(OperationRequest $request): void
    {
        $this->send($request->toArray());
    }

    public function registerSchedule(OperationRequest $request): void
    {
        $this->send($request->toArray());
    }

    public function removeSchedule(OperationRequest $request): void
    {
        $this->send($request->toArray());
    }

    public function registerDatabase(OperationRequest $request): array
    {
        return $this->send($request->toArray());
    }

    public function removeDatabase(OperationRequest $request): void
    {
        $this->send($request->toArray());
    }

    public function rotateDatabase(OperationRequest $request): void
    {
        $this->send($request->toArray());
    }

    public function registerCertificate(OperationRequest $request): void
    {
        $this->send($request->toArray());
    }

    public function applicationLogs(OperationRequest $request): ApplicationLogSnapshot
    {
        return ApplicationLogSnapshot::fromArray($this->send($request->toArray()));
    }

    public function applicationMetrics(OperationRequest $request): ApplicationMetricsSnapshot
    {
        return ApplicationMetricsSnapshot::fromArray($this->send($request->toArray()));
    }

    public function applicationObservation(OperationRequest $request): ApplicationObservation
    {
        return ApplicationObservation::fromArray($this->send($request->toArray()));
    }

    public function serverUpdates(OperationRequest $request): ServerUpdatesSnapshot
    {
        return ServerUpdatesSnapshot::fromArray($this->send($request->toArray()));
    }

    public function registerPackage(OperationRequest $request): void
    {
        $this->send($request->toArray());
    }

    public function removePackage(OperationRequest $request): void
    {
        $this->send($request->toArray());
    }

    public function registerBackup(OperationRequest $request): void
    {
        $this->send($request->toArray());
    }

    /**
     * @param  array<string, mixed>  $parameters
     * @return array<string, mixed>
     */
    private function call(string $operation, array $parameters = []): array
    {
        return $this->send([
            'protocol_version' => (int) config('hostnur.protocol_version', 1),
            'request_id' => (string) Str::uuid(),
            'idempotency_key' => (string) Str::uuid(),
            'operation' => $operation,
            'parameters' => $parameters === [] ? (object) [] : $parameters,
        ]);
    }

    /**
     * @param  array<string, mixed>  $payload
     * @return array<string, mixed>
     */
    private function send(array $payload): array
    {
        $response = $this->transport->request($payload);

        if (isset($response['error']) && is_array($response['error'])) {
            $error = AgentError::fromArray($response['error']);

            throw new AgentRemoteException($error->code, $error->message, $error->retryable);
        }

        return $response;
    }
}
