<?php

namespace Tests\Unit;

use App\Agent\AgentContractException;
use App\Agent\Dto\OperationRequest;
use App\Contracts\AgentClient;
use App\Enums\AgentMode;
use App\Enums\ErrorCode;
use Illuminate\Support\Carbon;
use Tests\TestCase;

class FakeAgentTest extends TestCase
{
    private function fakeClient(): AgentClient
    {
        config(['hostnur.agent_mode' => 'fake']);

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

    public function test_fake_capabilities_are_simulated_and_available(): void
    {
        $this->travelTo(Carbon::parse('2026-10-12T12:00:00Z'));

        $capabilities = $this->fakeClient()->capabilities();

        $this->assertSame(AgentMode::Fake, $capabilities->mode);
        $this->assertTrue($capabilities->available);
        $this->assertContains('server.snapshot', $capabilities->operations);
        $this->assertSame(now()->toIso8601String(), $capabilities->sampledAt);
        $this->assertNotNull($capabilities->reason);
    }

    public function test_fake_snapshot_returns_fixture_observations_with_mode_and_time(): void
    {
        $this->travelTo(Carbon::parse('2026-10-12T12:00:00Z'));

        $snapshot = $this->fakeClient()->snapshot();

        $this->assertSame(AgentMode::Fake, $snapshot->mode);
        $this->assertTrue($snapshot->available);
        $this->assertSame(now()->toIso8601String(), $snapshot->sampledAt);
        $this->assertCount(4, $snapshot->metrics);
        $this->assertCount(3, $snapshot->services);
        $this->assertSame('cpu_percent', $snapshot->metrics[0]['name']);
        $this->assertSame('running', $snapshot->services[0]['status']);
    }

    public function test_fake_snapshot_is_deterministic_at_a_frozen_time(): void
    {
        $this->freezeTime();

        $first = $this->fakeClient()->snapshot();
        $second = $this->fakeClient()->snapshot();

        $this->assertSame($first->toArray(), $second->toArray());
    }

    public function test_fake_snapshot_ages_into_stale(): void
    {
        $this->travelTo(Carbon::parse('2026-10-12T12:00:00Z'));

        $snapshot = $this->fakeClient()->snapshot();

        $this->assertFalse($snapshot->isStale(120));

        $this->travel(300)->seconds();

        $this->assertTrue($snapshot->isStale(120));
    }

    public function test_fake_and_offline_snapshots_are_distinguishable(): void
    {
        $fake = $this->fakeClient()->snapshot();

        config(['hostnur.agent_mode' => 'unavailable']);
        $offline = app(AgentClient::class)->snapshot();

        $this->assertSame(AgentMode::Fake, $fake->mode);
        $this->assertTrue($fake->available);
        $this->assertSame(AgentMode::Unavailable, $offline->mode);
        $this->assertFalse($offline->available);
    }

    public function test_fake_execution_is_not_implemented_yet(): void
    {
        $client = $this->fakeClient();

        foreach ([
            fn () => $client->submit($this->request()),
            fn () => $client->inspect('d1e2f3a4-b5c6-4d7e-8f9a-0b1c2d3e4f5a'),
            fn () => $client->cancel('d1e2f3a4-b5c6-4d7e-8f9a-0b1c2d3e4f5a'),
        ] as $call) {
            try {
                $call();
                $this->fail('Expected the fake execution API to be unavailable.');
            } catch (AgentContractException $exception) {
                $this->assertSame(ErrorCode::UnsupportedOperation, $exception->errorCode);
            }
        }
    }
}
