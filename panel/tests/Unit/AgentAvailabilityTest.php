<?php

namespace Tests\Unit;

use App\Agent\Dto\OperationRequest;
use App\Agent\Exceptions\AgentConfigurationException;
use App\Agent\Exceptions\AgentUnavailableException;
use App\Agent\FakeAgentClient;
use App\Agent\RealAgentClient;
use App\Agent\UnavailableAgentClient;
use App\Contracts\AgentClient;
use App\Enums\AgentMode;
use App\Enums\ErrorCode;
use Tests\TestCase;

class AgentAvailabilityTest extends TestCase
{
    private const EXECUTION_ID = 'd1e2f3a4-b5c6-4d7e-8f9a-0b1c2d3e4f5a';

    private function client(): AgentClient
    {
        return app(AgentClient::class);
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

    private function assertFailsClosed(callable $call): void
    {
        try {
            $call();
            $this->fail('Expected the unavailable agent to reject the operation.');
        } catch (AgentUnavailableException $exception) {
            $this->assertSame(ErrorCode::AgentUnavailable, $exception->errorCode());
        }
    }

    public function test_unavailable_is_the_default_mode(): void
    {
        $this->assertSame('unavailable', config('hostnur.agent_mode'));
        $this->assertInstanceOf(UnavailableAgentClient::class, $this->client());
    }

    public function test_unavailable_capabilities_fail_closed(): void
    {
        $capabilities = $this->client()->capabilities();

        $this->assertSame(AgentMode::Unavailable, $capabilities->mode);
        $this->assertFalse($capabilities->available);
        $this->assertSame([], $capabilities->operations);
        $this->assertNotNull($capabilities->reason);
    }

    public function test_unavailable_snapshot_reports_no_observations(): void
    {
        $snapshot = $this->client()->snapshot();

        $this->assertSame(AgentMode::Unavailable, $snapshot->mode);
        $this->assertFalse($snapshot->available);
        $this->assertSame([], $snapshot->metrics);
        $this->assertSame([], $snapshot->services);
        $this->assertNotNull($snapshot->reason);
    }

    public function test_mutations_fail_closed_when_unavailable(): void
    {
        $this->assertFailsClosed(fn () => $this->client()->submit($this->request()));
        $this->assertFailsClosed(fn () => $this->client()->inspect(self::EXECUTION_ID));
        $this->assertFailsClosed(fn () => $this->client()->cancel(self::EXECUTION_ID));
    }

    public function test_real_mode_resolves_to_the_real_client(): void
    {
        config(['hostnur.agent_mode' => 'real']);

        $client = $this->client();

        $this->assertInstanceOf(RealAgentClient::class, $client);
        $this->assertNotInstanceOf(FakeAgentClient::class, $client);
        $this->assertNotInstanceOf(UnavailableAgentClient::class, $client);
    }

    public function test_real_mode_is_allowed_in_production(): void
    {
        $this->app['env'] = 'production';
        config(['hostnur.agent_mode' => 'real']);

        $this->assertInstanceOf(RealAgentClient::class, $this->client());
    }

    public function test_fake_mode_is_denied_in_production(): void
    {
        $this->app['env'] = 'production';
        config(['hostnur.agent_mode' => 'fake']);

        try {
            $this->client();
            $this->fail('Expected fake mode to be rejected in production.');
        } catch (AgentConfigurationException $exception) {
            $this->assertStringContainsString('not allowed', $exception->getMessage());
        }
    }

    public function test_fake_mode_resolves_to_the_fake_client_in_testing(): void
    {
        config(['hostnur.agent_mode' => 'fake']);

        $this->assertInstanceOf(FakeAgentClient::class, $this->client());
    }

    public function test_an_unknown_mode_is_rejected(): void
    {
        config(['hostnur.agent_mode' => 'bogus']);

        try {
            $this->client();
            $this->fail('Expected the unknown mode to be rejected.');
        } catch (AgentConfigurationException $exception) {
            $this->assertStringContainsString('Unknown agent mode', $exception->getMessage());
        }
    }
}
