<?php

namespace Tests\Feature;

use App\Agent\AgentContractException;
use App\Agent\Dto\OperationRequest;
use App\Agent\Dto\ResourceRef;
use App\Enums\ErrorCode;
use App\Enums\ResourceKind;
use App\Enums\TaskStatus;
use App\Jobs\RunTask;
use App\Models\User;
use App\Services\RecentAuthentication;
use App\Services\TaskService;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Support\Facades\Queue;
use Tests\TestCase;

class TaskRequestTest extends TestCase
{
    use RefreshDatabase;

    private function admin(): User
    {
        return User::factory()->create(['email' => 'admin@example.com', 'is_admin' => true]);
    }

    private function operation(
        string $operation = 'server.snapshot',
        ?ResourceRef $resource = null,
        array $parameters = [],
        string $idempotencyKey = 'cf36a770-2abf-42e6-83cf-a0f042bcfc3c',
    ): OperationRequest {
        if ($resource === null && $operation === 'application.restart') {
            $resource = new ResourceRef(ResourceKind::Application, '224065ac-4bc7-4ba4-a06d-8c8bca950c32');
        }

        return new OperationRequest(
            protocolVersion: 1,
            requestId: '6c9dc595-fc9c-4c13-8c72-ed9a489ab9e7',
            idempotencyKey: $idempotencyKey,
            operation: $operation,
            resource: $resource,
            parameters: $parameters,
            timeoutSeconds: null,
        );
    }

    private function service(): TaskService
    {
        return app(TaskService::class);
    }

    public function test_a_read_only_request_is_queued_and_dispatched(): void
    {
        Queue::fake();

        $task = $this->service()->request($this->admin(), $this->operation());

        $this->assertSame(TaskStatus::Queued, $task->status);
        $this->assertDatabaseCount('tasks', 1);
        $this->assertDatabaseHas('audit_events', ['action' => 'task.requested', 'outcome' => 'requested']);
        Queue::assertPushed(RunTask::class, fn (RunTask $job): bool => $job->taskId === $task->getKey());
    }

    public function test_a_consequential_request_does_not_require_recent_authentication(): void
    {
        Queue::fake();

        // MFA login is sufficient; the operator is not asked to reconfirm the
        // password to reach privileged pages or submit consequential requests.
        $task = $this->service()->request($this->admin(), $this->operation('application.restart'));

        $this->assertSame(TaskStatus::AwaitingApproval, $task->status);
        $this->assertDatabaseCount('tasks', 1);
    }

    public function test_a_consequential_request_with_recent_auth_awaits_approval(): void
    {
        Queue::fake();
        $admin = $this->admin();
        app(RecentAuthentication::class)->confirm($admin);

        $task = $this->service()->request($admin, $this->operation('application.restart'));

        $this->assertSame(TaskStatus::AwaitingApproval, $task->status);
        Queue::assertNothingPushed();
    }

    public function test_double_submission_with_the_same_key_returns_the_same_intent(): void
    {
        Queue::fake();
        $admin = $this->admin();
        $request = $this->operation();

        $first = $this->service()->request($admin, $request);
        $second = $this->service()->request($admin, $request);

        $this->assertSame($first->getKey(), $second->getKey());
        $this->assertDatabaseCount('tasks', 1);
        Queue::assertPushed(RunTask::class, 1);
    }

    public function test_the_same_key_with_a_different_payload_conflicts(): void
    {
        Queue::fake();
        $admin = $this->admin();
        app(RecentAuthentication::class)->confirm($admin);

        $this->service()->request($admin, $this->operation('application.restart'));

        try {
            $this->service()->request($admin, $this->operation('application.deploy', parameters: ['revision' => '224065ac-4bc7-4ba4-a06d-8c8bca950c32']));
            $this->fail('Expected a conflicting idempotency key to be rejected.');
        } catch (AgentContractException $exception) {
            $this->assertSame(ErrorCode::Conflict, $exception->errorCode);
        }

        $this->assertDatabaseCount('tasks', 1);
    }

    public function test_approval_queues_the_task_and_records_audit(): void
    {
        Queue::fake();
        $admin = $this->admin();
        app(RecentAuthentication::class)->confirm($admin);
        $task = $this->service()->request($admin, $this->operation('application.restart'));

        $this->service()->approve($admin, $task);

        $task->refresh();
        $this->assertSame(TaskStatus::Queued, $task->status);
        $this->assertNotNull($task->approved_at);
        $this->assertSame($admin->getKey(), $task->approved_by);
        $this->assertDatabaseHas('audit_events', ['action' => 'task.approved']);
        Queue::assertPushed(RunTask::class);
    }

    public function test_denial_cancels_the_task(): void
    {
        Queue::fake();
        $admin = $this->admin();
        app(RecentAuthentication::class)->confirm($admin);
        $task = $this->service()->request($admin, $this->operation('application.restart'));

        $this->service()->deny($admin, $task, 'not_now');

        $task->refresh();
        $this->assertSame(TaskStatus::Cancelled, $task->status);
        $this->assertSame('not_now', $task->denied_reason_code);
        $this->assertDatabaseHas('audit_events', ['action' => 'task.denied', 'outcome' => 'denied']);
    }
}
