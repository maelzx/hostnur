<?php

namespace Tests\Feature;

use App\Enums\TaskStatus;
use App\Jobs\RunTask;
use App\Models\FakeAgentExecution;
use App\Models\Task;
use App\Models\User;
use App\Services\TaskService;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Support\Carbon;
use Illuminate\Support\Facades\Queue;
use Illuminate\Support\Str;
use Tests\TestCase;

class QueueInterruptionTest extends TestCase
{
    use RefreshDatabase;

    private function admin(): User
    {
        return User::factory()->create(['email' => 'admin@example.com', 'is_admin' => true]);
    }

    private function queuedTask(User $admin): Task
    {
        return Task::create([
            'actor_id' => $admin->getKey(),
            'actor_label' => $admin->email,
            'operation' => 'server.snapshot',
            'mode' => 'fake',
            'status' => TaskStatus::Queued,
            'idempotency_key' => (string) Str::uuid(),
            'parameters' => [],
            'payload_fingerprint' => str_repeat('a', 64),
            'requested_at' => now(),
            'queued_at' => now(),
        ]);
    }

    private function service(): TaskService
    {
        return app(TaskService::class);
    }

    public function test_a_committed_intent_is_redispatched_by_reconciliation(): void
    {
        // Simulate a crash between commit and enqueue: the task exists but no job
        // was ever dispatched.
        $admin = $this->admin();
        $task = $this->queuedTask($admin);

        Queue::fake();
        $result = $this->service()->reconcile();

        $this->assertSame(1, $result['dispatched']);
        Queue::assertPushed(RunTask::class, fn (RunTask $job): bool => $job->taskId === $task->getKey());
        $this->assertDatabaseHas('tasks', ['id' => $task->getKey(), 'status' => TaskStatus::Queued->value]);
    }

    public function test_a_concurrent_worker_cannot_double_submit(): void
    {
        config(['hostnur.agent_mode' => 'fake']);
        $this->travelTo(Carbon::parse('2026-10-13T06:00:00Z'));

        $admin = $this->admin();
        $task = $this->queuedTask($admin);
        $this->service()->run($task->getKey());
        $task->refresh();
        $executionId = $task->execution_id;

        Task::query()->whereKey($task->getKey())->update([
            'lease_token' => 'held',
            'lease_expires_at' => now()->addMinute(),
        ]);

        $this->service()->run($task->getKey());
        $task->refresh();

        $this->assertSame($executionId, $task->execution_id);
        $this->assertSame(1, $task->events()->where('type', 'submitted')->count());
    }

    public function test_an_unknown_execution_is_not_success_and_is_not_replayed(): void
    {
        config(['hostnur.agent_mode' => 'fake']);
        $this->travelTo(Carbon::parse('2026-10-13T06:00:00Z'));

        $admin = $this->admin();
        $task = $this->queuedTask($admin);
        $this->service()->run($task->getKey());
        $task->refresh();

        FakeAgentExecution::query()->delete();
        $this->service()->run($task->getKey());
        $task->refresh();

        $this->assertSame(TaskStatus::Unknown, $task->status);
        $this->assertSame(1, $task->events()->where('type', 'submitted')->count());
        $this->assertDatabaseCount('tasks', 1);
    }

    public function test_cancellation_is_accurate_and_does_not_replay(): void
    {
        config(['hostnur.agent_mode' => 'fake']);
        $admin = $this->admin();
        $task = $this->queuedTask($admin);

        $this->service()->cancel($admin, $task);
        $this->service()->run($task->getKey());
        $task->refresh();

        $this->assertSame(TaskStatus::Cancelled, $task->status);
        $this->assertSame(0, $task->events()->where('type', 'submitted')->count());

        // Cancelling an already-finished task changes nothing.
        $task->forceFill(['status' => TaskStatus::Succeeded, 'finished_at' => now()])->save();
        $this->service()->cancel($admin, $task->refresh());
        $this->assertSame(TaskStatus::Succeeded, $task->refresh()->status);
    }
}
