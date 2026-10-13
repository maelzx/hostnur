<?php

namespace Tests\Feature;

use App\Enums\AuditAction;
use App\Enums\AuditOutcome;
use App\Enums\TaskEventLevel;
use App\Enums\TaskStatus;
use App\Livewire\Panel\AuditIndex;
use App\Livewire\Panel\TaskIndex;
use App\Livewire\Panel\TaskShow;
use App\Models\AuditEvent;
use App\Models\Task;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Support\Str;
use Livewire\Livewire;
use Tests\Support\InteractsWithPanel;
use Tests\TestCase;

class TaskVisibilityTest extends TestCase
{
    use InteractsWithPanel, RefreshDatabase;

    private function makeTask(array $attributes = []): Task
    {
        return Task::create(array_merge([
            'actor_label' => 'admin@example.com',
            'operation' => 'server.snapshot',
            'mode' => 'fake',
            'status' => TaskStatus::Succeeded,
            'idempotency_key' => (string) Str::uuid(),
            'parameters' => [],
            'payload_fingerprint' => str_repeat('a', 64),
            'requested_at' => now(),
        ], $attributes));
    }

    public function test_the_task_index_filters_by_status(): void
    {
        $this->makeTask(['operation' => 'server.snapshot', 'status' => TaskStatus::Succeeded]);
        $this->makeTask(['operation' => 'application.deploy', 'status' => TaskStatus::Failed]);

        Livewire::test(TaskIndex::class)
            ->assertSee('server.snapshot')
            ->assertSee('application.deploy')
            ->set('status', 'failed')
            ->assertSee('application.deploy')
            ->assertDontSee('server.snapshot');
    }

    public function test_the_task_index_filters_by_operation(): void
    {
        $this->makeTask(['operation' => 'server.snapshot']);
        $this->makeTask(['operation' => 'application.deploy']);

        Livewire::test(TaskIndex::class)
            ->set('operation', 'deploy')
            ->assertSee('application.deploy')
            ->assertDontSee('server.snapshot');
    }

    public function test_the_task_detail_paginates_events_with_load_more(): void
    {
        $task = $this->makeTask();

        for ($index = 0; $index < 60; $index++) {
            $task->recordEvent('output', TaskEventLevel::Info, "event {$index}");
        }

        Livewire::test(TaskShow::class, ['task' => $task->getKey()])
            ->assertSet('eventLimit', 50)
            ->assertSee('event 0')
            ->assertDontSee('event 59')
            ->call('loadMoreEvents')
            ->assertSet('eventLimit', 100)
            ->assertSee('event 59');
    }

    public function test_the_task_detail_polls_only_while_active(): void
    {
        $admin = $this->confirmedAdmin();
        $active = $this->makeTask(['status' => TaskStatus::Running]);
        $terminal = $this->makeTask(['status' => TaskStatus::Succeeded]);

        $this->actingAs($admin)
            ->withSession($this->mfaProof($admin))
            ->get(route('panel.tasks.show', ['task' => $active->getKey()]))
            ->assertOk()
            ->assertSee('wire:poll.5s', false);

        $this->actingAs($admin)
            ->withSession($this->mfaProof($admin))
            ->get(route('panel.tasks.show', ['task' => $terminal->getKey()]))
            ->assertOk()
            ->assertDontSee('wire:poll.5s', false);
    }

    public function test_the_audit_index_filters_by_action_and_shows_detail(): void
    {
        $login = AuditEvent::create(['actor_label' => 'admin@example.com', 'action' => AuditAction::AuthLogin, 'outcome' => AuditOutcome::Success, 'recorded_at' => now()]);
        $target = AuditEvent::create(['actor_label' => 'admin@example.com', 'action' => AuditAction::TaskRequested, 'outcome' => AuditOutcome::Requested, 'recorded_at' => now(), 'metadata' => ['operation' => 'application.restart']]);

        Livewire::test(AuditIndex::class)
            ->assertSee('wire:key="audit-'.$login->getKey().'"', false)
            ->assertSee('wire:key="audit-'.$target->getKey().'"', false)
            ->set('action', 'task.requested')
            ->assertSee('wire:key="audit-'.$target->getKey().'"', false)
            ->assertDontSee('wire:key="audit-'.$login->getKey().'"', false);

        $admin = $this->confirmedAdmin();

        $this->actingAs($admin)
            ->withSession($this->mfaProof($admin))
            ->get(route('panel.audit.show', ['event' => $target->getKey()]))
            ->assertOk()
            ->assertSee('task.requested')
            ->assertSee('application.restart');
    }
}
