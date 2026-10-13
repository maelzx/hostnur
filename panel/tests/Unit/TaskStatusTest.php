<?php

namespace Tests\Unit;

use App\Enums\TaskEventLevel;
use App\Enums\TaskStatus;
use App\Exceptions\InvalidTaskTransitionException;
use App\Models\Task;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Tests\TestCase;

class TaskStatusTest extends TestCase
{
    use RefreshDatabase;

    public function test_terminal_and_active_states_are_classified(): void
    {
        $this->assertTrue(TaskStatus::Succeeded->isTerminal());
        $this->assertTrue(TaskStatus::Failed->isTerminal());
        $this->assertTrue(TaskStatus::Cancelled->isTerminal());
        $this->assertFalse(TaskStatus::Unknown->isTerminal());
        $this->assertTrue(TaskStatus::Running->isActive());
        $this->assertFalse(TaskStatus::Succeeded->isActive());
    }

    public function test_allowed_transitions_are_enforced(): void
    {
        $this->assertTrue(TaskStatus::Queued->canTransitionTo(TaskStatus::Running));
        $this->assertTrue(TaskStatus::Running->canTransitionTo(TaskStatus::Succeeded));
        $this->assertTrue(TaskStatus::Unknown->canTransitionTo(TaskStatus::Succeeded));
        $this->assertFalse(TaskStatus::Succeeded->canTransitionTo(TaskStatus::Running));
        $this->assertFalse(TaskStatus::Running->canTransitionTo(TaskStatus::Queued));
        $this->assertFalse(TaskStatus::Cancelled->canTransitionTo(TaskStatus::Running));
    }

    public function test_a_late_transition_on_a_terminal_task_is_rejected(): void
    {
        $task = Task::create([
            'actor_label' => 'admin@example.com',
            'operation' => 'server.snapshot',
            'mode' => 'unavailable',
            'status' => TaskStatus::Succeeded,
            'idempotency_key' => 'cf36a770-2abf-42e6-83cf-a0f042bcfc3c',
            'parameters' => [],
            'payload_fingerprint' => str_repeat('a', 64),
            'requested_at' => now(),
        ]);

        $this->expectException(InvalidTaskTransitionException::class);

        $task->transitionTo(TaskStatus::Running);
    }

    public function test_task_events_are_sequential_and_relationship_is_preserved(): void
    {
        $task = Task::create([
            'actor_label' => 'admin@example.com',
            'operation' => 'server.snapshot',
            'mode' => 'fake',
            'status' => TaskStatus::Queued,
            'idempotency_key' => 'cf36a770-2abf-42e6-83cf-a0f042bcfc3c',
            'parameters' => [],
            'payload_fingerprint' => str_repeat('b', 64),
            'requested_at' => now(),
        ]);

        $task->recordEvent('requested', TaskEventLevel::Info, 'Requested.');
        $task->recordEvent('queued', TaskEventLevel::Info, 'Queued.');

        $this->assertSame([1, 2], $task->events()->pluck('sequence')->all());
    }
}
