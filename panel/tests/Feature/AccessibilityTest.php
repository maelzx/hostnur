<?php

namespace Tests\Feature;

use App\Enums\AuditAction;
use App\Enums\AuditOutcome;
use App\Enums\TaskStatus;
use App\Models\AuditEvent;
use App\Models\Task;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Support\Str;
use Tests\Support\InteractsWithPanel;
use Tests\TestCase;

class AccessibilityTest extends TestCase
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

    public function test_the_shell_has_a_skip_link_and_labelled_navigation(): void
    {
        config(['hostnur.agent_mode' => 'unavailable']);
        $admin = $this->confirmedAdmin();

        $this->actingAs($admin)
            ->withSession($this->mfaProof($admin))
            ->get('/panel')
            ->assertOk()
            ->assertSee('Skip to content')
            ->assertSee('href="#main-content"', false)
            ->assertSee('id="main-content"', false)
            ->assertSee('aria-label="Panel"', false);
    }

    public function test_tables_are_scrollable_and_use_captions_and_scoped_headers(): void
    {
        config(['hostnur.agent_mode' => 'unavailable']);
        $admin = $this->confirmedAdmin();
        $this->makeTask();
        AuditEvent::create(['actor_label' => 'admin@example.com', 'action' => AuditAction::AuthLogin, 'outcome' => AuditOutcome::Success, 'recorded_at' => now()]);

        $this->actingAs($admin)
            ->withSession($this->mfaProof($admin))
            ->get('/panel/tasks')
            ->assertOk()
            ->assertSee('overflow-x-auto', false)
            ->assertSee('<caption', false)
            ->assertSee('scope="col"', false)
            ->assertSee('scope="row"', false);

        $this->actingAs($admin)
            ->withSession($this->mfaProof($admin))
            ->get('/panel/audit')
            ->assertOk()
            ->assertSee('scope="col"', false);
    }

    public function test_the_confirmation_dialog_is_modal_and_labelled(): void
    {
        config(['hostnur.agent_mode' => 'unavailable']);
        $admin = $this->confirmedAdmin();

        $this->actingAs($admin)
            ->withSession(array_merge($this->mfaProof($admin), $this->recentAuth($admin)))
            ->get('/panel/security')
            ->assertOk()
            ->assertSee('role="dialog"', false)
            ->assertSee('aria-modal="true"', false)
            ->assertSee('aria-labelledby', false);
    }

    public function test_task_status_is_conveyed_by_text_not_only_colour(): void
    {
        config(['hostnur.agent_mode' => 'unavailable']);
        $admin = $this->confirmedAdmin();
        $this->makeTask(['status' => TaskStatus::Succeeded]);

        $this->actingAs($admin)
            ->withSession($this->mfaProof($admin))
            ->get('/panel/tasks')
            ->assertOk()
            ->assertSee('succeeded');
    }
}
