<?php

namespace Tests\Unit;

use App\Agent\Dto\OperationRequest;
use App\Agent\Exceptions\AgentRemoteException;
use App\Agent\RealAgentClient;
use App\Agent\Transport\AgentTransport;
use App\Enums\ErrorCode;
use Tests\TestCase;

class RealAgentClientTest extends TestCase
{
    /**
     * @param  array<string, mixed>  $response
     */
    private function client(array $response): RealAgentClient
    {
        return new RealAgentClient($this->transport($response));
    }

    /**
     * @param  array<string, mixed>  $response
     */
    private function transport(array $response): AgentTransport
    {
        return new class($response) implements AgentTransport
        {
            /** @var list<array<string, mixed>> */
            public array $requests = [];

            /**
             * @param  array<string, mixed>  $response
             */
            public function __construct(private readonly array $response) {}

            public function request(array $payload): array
            {
                $this->requests[] = $payload;

                return $this->response;
            }
        };
    }

    private function request(): OperationRequest
    {
        return new OperationRequest(
            protocolVersion: 1,
            requestId: '6c9dc595-fc9c-4c13-8c72-ed9a489ab9e7',
            idempotencyKey: 'cf36a770-2abf-42e6-83cf-a0f042bcfc3c',
            operation: 'server.snapshot',
            resource: null,
            parameters: [],
            timeoutSeconds: null,
        );
    }

    public function test_capabilities_uses_the_control_operation(): void
    {
        $transport = $this->transport([
            'protocol_version' => 1,
            'mode' => 'real',
            'available' => true,
            'operations' => ['agent.inspect'],
            'sampled_at' => '2026-10-12T12:00:00Z',
            'reason' => null,
        ]);

        $capabilities = (new RealAgentClient($transport))->capabilities();

        $this->assertTrue($capabilities->available);
        $this->assertSame(['agent.inspect'], $capabilities->operations);
        $this->assertSame('agent.capabilities', $transport->requests[0]['operation']);
    }

    public function test_submit_sends_the_validated_envelope(): void
    {
        $transport = $this->transport([
            'protocol_version' => 1,
            'request_id' => '6c9dc595-fc9c-4c13-8c72-ed9a489ab9e7',
            'execution_id' => 'd1e2f3a4-b5c6-4d7e-8f9a-0b1c2d3e4f5a',
            'operation' => 'server.snapshot',
            'status' => 'accepted',
            'accepted_at' => '2026-10-12T12:00:00Z',
        ]);

        $receipt = (new RealAgentClient($transport))->submit($this->request());

        $this->assertSame('accepted', $receipt->status->value);
        $this->assertSame('server.snapshot', $transport->requests[0]['operation']);
        $this->assertSame('{}', json_encode($transport->requests[0]['parameters']));
    }

    public function test_inspect_carries_the_execution_id(): void
    {
        $transport = $this->transport([
            'protocol_version' => 1,
            'execution_id' => 'd1e2f3a4-b5c6-4d7e-8f9a-0b1c2d3e4f5a',
            'request_id' => '0f6c5f2a-4c4b-4f1e-9c9b-2f1c8a7d6e55',
            'operation' => 'server.snapshot',
            'status' => 'running',
            'progress' => 10,
            'events' => [],
            'result' => null,
            'error' => null,
            'updated_at' => '2026-10-12T12:00:03Z',
        ]);

        $snapshot = (new RealAgentClient($transport))->inspect('d1e2f3a4-b5c6-4d7e-8f9a-0b1c2d3e4f5a');

        $this->assertSame('running', $snapshot->status->value);
        $this->assertSame('agent.inspect', $transport->requests[0]['operation']);
        $this->assertSame(
            'd1e2f3a4-b5c6-4d7e-8f9a-0b1c2d3e4f5a',
            $transport->requests[0]['parameters']['execution_id'],
        );
    }

    public function test_remote_error_is_raised(): void
    {
        $client = $this->client([
            'error' => [
                'code' => 'execution_unknown',
                'message' => 'The execution is not known to the agent.',
                'retryable' => false,
            ],
        ]);

        try {
            $client->inspect('d1e2f3a4-b5c6-4d7e-8f9a-0b1c2d3e4f5a');
            $this->fail('Expected a remote error.');
        } catch (AgentRemoteException $exception) {
            $this->assertSame(ErrorCode::ExecutionUnknown, $exception->errorCode());
            $this->assertFalse($exception->isRetryable());
        }
    }
}
