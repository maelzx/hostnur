<?php

namespace Tests\Unit;

use App\Agent\AgentContractException;
use App\Agent\Dto\OperationRequest;
use App\Agent\Dto\ResourceRef;
use App\Contracts\AgentClient;
use App\Enums\AgentMode;
use App\Enums\CancellationStatus;
use App\Enums\ErrorCode;
use App\Enums\OperationStatus;
use App\Enums\ResourceKind;
use App\Models\FakeAgentExecution;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Support\Carbon;
use Tests\TestCase;

class FakeAgentTest extends TestCase
{
    use RefreshDatabase;

    private function fakeClient(): AgentClient
    {
        config(['hostnur.agent_mode' => 'fake']);

        return app(AgentClient::class);
    }

    private function request(
        string $operation = 'application.restart',
        ?ResourceRef $resource = null,
        array $parameters = [],
        string $idempotencyKey = 'cf36a770-2abf-42e6-83cf-a0f042bcfc3c',
        string $requestId = '6c9dc595-fc9c-4c13-8c72-ed9a489ab9e7',
    ): OperationRequest {
        if ($resource === null && $operation === 'application.restart') {
            $resource = new ResourceRef(ResourceKind::Application, '224065ac-4bc7-4ba4-a06d-8c8bca950c32');
        }

        return new OperationRequest(
            protocolVersion: 1,
            requestId: $requestId,
            idempotencyKey: $idempotencyKey,
            operation: $operation,
            resource: $resource,
            parameters: $parameters,
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
        $this->assertContains('application.restart', $capabilities->operations);
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
        $this->assertCount(6, $snapshot->metrics);
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

    public function test_fake_submit_is_durable_and_idempotent(): void
    {
        $client = $this->fakeClient();
        $request = $this->request();

        $first = $client->submit($request);
        $second = $client->submit($request);

        $this->assertSame(OperationStatus::Accepted, $first->status);
        $this->assertSame($first->executionId, $second->executionId);
        $this->assertSame(1, FakeAgentExecution::query()->count());
    }

    public function test_fake_execution_progresses_from_running_to_succeeded(): void
    {
        $this->travelTo(Carbon::parse('2026-10-12T12:00:00Z'));

        $client = $this->fakeClient();
        $receipt = $client->submit($this->request());

        $this->travel(1)->seconds();
        $running = $client->inspect($receipt->executionId);
        $this->assertSame(OperationStatus::Running, $running->status);
        $this->assertSame(50, $running->progress);
        $this->assertNull($running->error);

        $this->travel(1)->seconds();
        $succeeded = $client->inspect($receipt->executionId);
        $this->assertSame(OperationStatus::Succeeded, $succeeded->status);
        $this->assertSame(100, $succeeded->progress);
        $this->assertNull($succeeded->error);
        $this->assertSame('application.restart', $succeeded->result['operation'] ?? null);
    }

    public function test_fake_failure_scenario_fails(): void
    {
        config(['hostnur.fake.scenario' => 'failure']);
        $this->travelTo(Carbon::parse('2026-10-12T12:00:00Z'));

        $client = $this->fakeClient();
        $receipt = $client->submit($this->request());

        $this->travel(2)->seconds();
        $snapshot = $client->inspect($receipt->executionId);

        $this->assertSame(OperationStatus::Failed, $snapshot->status);
        $this->assertSame(ErrorCode::OperationFailed, $snapshot->error?->code);
    }

    public function test_fake_timeout_scenario_times_out(): void
    {
        config(['hostnur.fake.scenario' => 'timeout']);
        $this->travelTo(Carbon::parse('2026-10-12T12:00:00Z'));

        $client = $this->fakeClient();
        $receipt = $client->submit($this->request());

        $this->travel(1)->seconds();
        $this->assertSame(OperationStatus::Running, $client->inspect($receipt->executionId)->status);

        $this->travel(2)->seconds();
        $timedOut = $client->inspect($receipt->executionId);
        $this->assertSame(OperationStatus::TimedOut, $timedOut->status);
        $this->assertSame(ErrorCode::Timeout, $timedOut->error?->code);
    }

    public function test_fake_inspect_of_an_unknown_execution_is_explicit(): void
    {
        try {
            $this->fakeClient()->inspect('d1e2f3a4-b5c6-4d7e-8f9a-0b1c2d3e4f5a');
            $this->fail('Expected an unknown execution to be reported.');
        } catch (AgentContractException $exception) {
            $this->assertSame(ErrorCode::ExecutionUnknown, $exception->errorCode);
        }
    }

    public function test_fake_cancel_active_execution_is_requested_then_cancelled(): void
    {
        $client = $this->fakeClient();
        $receipt = $client->submit($this->request());

        $cancellation = $client->cancel($receipt->executionId);
        $this->assertSame(CancellationStatus::Requested, $cancellation->status);

        $snapshot = $client->inspect($receipt->executionId);
        $this->assertSame(OperationStatus::Cancelled, $snapshot->status);
    }

    public function test_fake_cancel_finished_execution_is_already_finished(): void
    {
        $this->travelTo(Carbon::parse('2026-10-12T12:00:00Z'));

        $client = $this->fakeClient();
        $receipt = $client->submit($this->request());

        $this->travel(2)->seconds();

        $this->assertSame(
            CancellationStatus::AlreadyFinished,
            $client->cancel($receipt->executionId)->status,
        );
    }

    public function test_fake_cancel_of_a_read_only_operation_is_unsupported(): void
    {
        $client = $this->fakeClient();
        $receipt = $client->submit($this->request(operation: 'server.snapshot'));

        $this->assertSame(
            CancellationStatus::Unsupported,
            $client->cancel($receipt->executionId)->status,
        );
    }

    public function test_fake_submit_with_a_conflicting_idempotency_key_is_rejected(): void
    {
        $client = $this->fakeClient();

        $client->submit($this->request());

        try {
            $client->submit($this->request(operation: 'server.snapshot', resource: null));
            $this->fail('Expected the conflicting idempotency key to be rejected.');
        } catch (AgentContractException $exception) {
            $this->assertSame(ErrorCode::Conflict, $exception->errorCode);
        }
    }
}
