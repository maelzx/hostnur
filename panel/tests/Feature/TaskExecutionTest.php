<?php

namespace Tests\Feature;

use App\Agent\Dto\OperationRequest;
use App\Agent\Dto\ResourceRef;
use App\Enums\ResourceKind;
use App\Enums\TaskStatus;
use App\Jobs\RunTask;
use App\Models\FakeAgentExecution;
use App\Models\Task;
use App\Models\User;
use App\Services\RecentAuthentication;
use App\Services\TaskService;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Support\Carbon;
use Illuminate\Support\Facades\Queue;
use Tests\TestCase;

class TaskExecutionTest extends TestCase
{
    use RefreshDatabase;

    private function admin(): User
    {
        return User::factory()->create(['email' => 'admin@example.com', 'is_admin' => true]);
    }

    private function snapshotRequest(): OperationRequest
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

    private function restartRequest(string $key = 'cf36a770-2abf-42e6-83cf-a0f042bcfc3c'): OperationRequest
    {
        return new OperationRequest(
            protocolVersion: 1,
            requestId: '0f6c5f2a-4c4b-4f1e-9c9b-2f1c8a7d6e55',
            idempotencyKey: $key,
            operation: 'application.restart',
            resource: new ResourceRef(ResourceKind::Application, '224065ac-4bc7-4ba4-a06d-8c8bca950c32'),
            parameters: [],
            timeoutSeconds: null,
        );
    }

    private function service(): TaskService
    {
        return app(TaskService::class);
    }

    private function queuedReadOnlyTask(User $admin): Task
    {
        Queue::fake();

        return $this->service()->request($admin, $this->snapshotRequest());
    }

    public function test_a_successful_fake_execution_completes(): void
    {
        config(['hostnur.agent_mode' => 'fake']);
        $this->travelTo(Carbon::parse('2026-10-13T02:00:00Z'));
        $admin = $this->admin();
        $task = $this->queuedReadOnlyTask($admin);

        $this->service()->run($task->getKey());
        $task->refresh();
        $this->assertSame(TaskStatus::Running, $task->status);
        $this->assertNotNull($task->execution_id);
        $this->assertSame('fake', $task->mode);

        $this->travel(2)->seconds();
        $this->service()->run($task->getKey());
        $task->refresh();
        $this->assertSame(TaskStatus::Succeeded, $task->status);
        $this->assertNotNull($task->finished_at);
        $this->assertNotNull($task->result);
        $this->assertNull($task->lease_token);
        $this->assertDatabaseHas('audit_events', ['action' => 'task.succeeded']);
    }

    public function test_the_failure_scenario_fails_the_task(): void
    {
        config(['hostnur.agent_mode' => 'fake', 'hostnur.fake.scenario' => 'failure']);
        $this->travelTo(Carbon::parse('2026-10-13T02:00:00Z'));
        $task = $this->queuedReadOnlyTask($this->admin());

        $this->service()->run($task->getKey());
        $this->travel(2)->seconds();
        $this->service()->run($task->getKey());

        $task->refresh();
        $this->assertSame(TaskStatus::Failed, $task->status);
        $this->assertSame('operation_failed', $task->error_code);
    }

    public function test_the_timeout_scenario_fails_with_timeout(): void
    {
        config(['hostnur.agent_mode' => 'fake', 'hostnur.fake.scenario' => 'timeout']);
        $this->travelTo(Carbon::parse('2026-10-13T02:00:00Z'));
        $task = $this->queuedReadOnlyTask($this->admin());

        $this->service()->run($task->getKey());
        $this->travel(3)->seconds();
        $this->service()->run($task->getKey());

        $task->refresh();
        $this->assertSame(TaskStatus::Failed, $task->status);
        $this->assertSame('timeout', $task->error_code);
    }

    public function test_a_concurrent_worker_cannot_double_submit(): void
    {
        config(['hostnur.agent_mode' => 'fake']);
        $this->travelTo(Carbon::parse('2026-10-13T02:00:00Z'));
        $task = $this->queuedReadOnlyTask($this->admin());

        $this->service()->run($task->getKey());
        $task->refresh();
        $executionId = $task->execution_id;

        Task::query()->whereKey($task->getKey())->update([
            'lease_token' => 'held-by-another-worker',
            'lease_expires_at' => now()->addMinute(),
        ]);

        $this->service()->run($task->getKey());
        $task->refresh();

        $this->assertSame($executionId, $task->execution_id);
        $this->assertSame(1, $task->events()->where('type', 'submitted')->count());
    }

    public function test_an_unknown_execution_is_recorded_as_unknown(): void
    {
        config(['hostnur.agent_mode' => 'fake']);
        $this->travelTo(Carbon::parse('2026-10-13T02:00:00Z'));
        $task = $this->queuedReadOnlyTask($this->admin());

        $this->service()->run($task->getKey());
        $task->refresh();

        FakeAgentExecution::query()->where('execution_id', $task->execution_id)->delete();

        $this->service()->run($task->getKey());
        $task->refresh();

        $this->assertSame(TaskStatus::Unknown, $task->status);
        $this->assertDatabaseHas('audit_events', ['action' => 'task.unknown']);
    }

    public function test_cancelling_before_submission_cancels_the_task(): void
    {
        config(['hostnur.agent_mode' => 'fake']);
        $admin = $this->admin();
        $task = $this->queuedReadOnlyTask($admin);

        $this->service()->cancel($admin, $task);
        $this->service()->run($task->getKey());

        $task->refresh();
        $this->assertSame(TaskStatus::Cancelled, $task->status);
    }

    public function test_cancelling_an_awaiting_task_cancels_immediately(): void
    {
        Queue::fake();
        $admin = $this->admin();
        app(RecentAuthentication::class)->confirm($admin);
        $task = $this->service()->request($admin, $this->restartRequest());

        $this->service()->cancel($admin, $task);

        $this->assertSame(TaskStatus::Cancelled, $task->refresh()->status);
    }

    public function test_a_revoked_actor_cannot_submit_queued_work(): void
    {
        config(['hostnur.agent_mode' => 'fake']);
        $admin = $this->admin();
        $task = $this->queuedReadOnlyTask($admin);

        $admin->forceFill(['is_admin' => false])->save();

        $this->service()->run($task->getKey());

        $task->refresh();
        $this->assertSame(TaskStatus::Failed, $task->status);
        $this->assertSame('permission_denied', $task->error_code);
    }

    public function test_an_expired_approval_returns_the_task_to_awaiting(): void
    {
        Queue::fake();
        $admin = $this->admin();
        app(RecentAuthentication::class)->confirm($admin);
        $task = $this->service()->request($admin, $this->restartRequest());
        $this->service()->approve($admin, $task);

        Task::query()->whereKey($task->getKey())->update(['approval_expires_at' => now()->subMinute()]);

        $this->service()->run($task->getKey());

        $this->assertSame(TaskStatus::AwaitingApproval, $task->refresh()->status);
        $this->assertDatabaseHas('audit_events', ['action' => 'task.approval_expired']);
    }

    public function test_retry_creates_a_new_linked_task(): void
    {
        config(['hostnur.agent_mode' => 'fake', 'hostnur.fake.scenario' => 'failure']);
        $this->travelTo(Carbon::parse('2026-10-13T02:00:00Z'));
        $admin = $this->admin();
        $task = $this->queuedReadOnlyTask($admin);
        $this->service()->run($task->getKey());
        $this->travel(2)->seconds();
        $this->service()->run($task->getKey());
        $task->refresh();
        $this->assertSame(TaskStatus::Failed, $task->status);

        $retry = $this->service()->retry($admin, $task, '9a1b2c3d-4e5f-4a6b-8c7d-1e2f3a4b5c6d');

        $this->assertNotSame($task->getKey(), $retry->getKey());
        $this->assertDatabaseHas('audit_events', ['action' => 'task.retried']);
        $this->assertSame(1, $retry->events()->where('type', 'retry')->count());
    }

    public function test_reconciliation_dispatches_queued_tasks_and_recovers_expired_leases(): void
    {
        config(['hostnur.agent_mode' => 'fake']);
        $admin = $this->admin();
        $task = $this->queuedReadOnlyTask($admin);

        Task::query()->whereKey($task->getKey())->update(['lease_expires_at' => now()->subMinute()]);

        Queue::fake();
        $result = $this->service()->reconcile();

        $this->assertSame(1, $result['dispatched']);
        $this->assertSame(1, $result['leases_expired']);
        Queue::assertPushed(RunTask::class);
        $this->assertNull($task->refresh()->lease_token);
    }
}
