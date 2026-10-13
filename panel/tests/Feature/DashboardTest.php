<?php

namespace Tests\Feature;

use App\Enums\TaskStatus;
use App\Models\Task;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Support\Str;
use Tests\Support\InteractsWithPanel;
use Tests\TestCase;

class DashboardTest extends TestCase
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

    public function test_the_dashboard_labels_simulated_data_in_fake_mode(): void
    {
        config(['hostnur.agent_mode' => 'fake']);
        $admin = $this->confirmedAdmin();

        $this->actingAs($admin)
            ->withSession($this->mfaProof($admin))
            ->get('/panel')
            ->assertOk()
            ->assertSee('Simulated data (fake agent mode)')
            ->assertSee('Ubuntu 24.04 LTS')
            ->assertSee('hostnur-demo')
            ->assertSee('12.4');
    }

    public function test_the_dashboard_shows_unavailable_without_zero_metrics(): void
    {
        config(['hostnur.agent_mode' => 'unavailable']);
        $admin = $this->confirmedAdmin();

        $this->actingAs($admin)
            ->withSession($this->mfaProof($admin))
            ->get('/panel')
            ->assertOk()
            ->assertSee('The agent is unavailable')
            ->assertSee('—')
            ->assertDontSee('Ubuntu 24.04 LTS')
            ->assertDontSee('12.4');
    }

    public function test_the_dashboard_caches_plain_data_on_the_database_store(): void
    {
        config(['hostnur.agent_mode' => 'fake', 'cache.default' => 'database']);
        $admin = $this->confirmedAdmin();

        $this->actingAs($admin)
            ->withSession($this->mfaProof($admin))
            ->get('/panel')
            ->assertOk()
            ->assertSee('Simulated data (fake agent mode)');

        $this->actingAs($admin)
            ->withSession($this->mfaProof($admin))
            ->get('/panel')
            ->assertOk()
            ->assertSee('Ubuntu 24.04 LTS');
    }

    public function test_the_dashboard_reports_active_tasks_and_recent_failures(): void
    {
        config(['hostnur.agent_mode' => 'unavailable']);
        $admin = $this->confirmedAdmin();

        $this->makeTask(['operation' => 'application.deploy', 'status' => TaskStatus::Failed, 'error_code' => 'operation_failed', 'finished_at' => now()]);
        $this->makeTask(['operation' => 'application.restart', 'status' => TaskStatus::Running]);

        $this->actingAs($admin)
            ->withSession($this->mfaProof($admin))
            ->get('/panel')
            ->assertOk()
            ->assertSee('Active tasks')
            ->assertSee('application.deploy')
            ->assertSee('operation_failed');
    }
}
