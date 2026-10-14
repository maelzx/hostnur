<?php

namespace Tests\Feature;

use App\Enums\TaskEventLevel;
use App\Enums\TaskStatus;
use App\Jobs\RunTask;
use App\Livewire\Panel\ApplicationOperations;
use App\Models\Application;
use App\Models\ApplicationSecret;
use App\Models\Task;
use App\Models\User;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Support\Str;
use Livewire\Livewire;
use Tests\Support\InteractsWithPanel;
use Tests\TestCase;

class WebRegressionTest extends TestCase
{
    use InteractsWithPanel, RefreshDatabase;

    private function application(User $admin): Application
    {
        return Application::create([
            'name' => 'My App',
            'slug' => 'my-app-'.Str::lower(Str::random(4)),
            'provider' => 'laravel',
            'runtime' => 'php',
            'runtime_version' => '8.3',
            'source_status' => 'not_checked',
            'current_revision' => 1,
        ]);
    }

    private function task(): Task
    {
        return Task::create([
            'actor_label' => 'admin@example.com',
            'operation' => 'application.deploy',
            'resource_kind' => 'application',
            'resource_id' => (string) Str::uuid(),
            'mode' => 'fake',
            'status' => TaskStatus::Succeeded,
            'idempotency_key' => (string) Str::uuid(),
            'parameters' => [],
            'payload_fingerprint' => str_repeat('a', 64),
            'requested_at' => now(),
        ]);
    }

    public function test_guests_cannot_reach_any_panel_route(): void
    {
        foreach ([
            'panel.dashboard', 'panel.tasks.index', 'panel.audit.index',
            'panel.applications.index', 'panel.deployments.index', 'panel.server',
            'panel.schedules.index', 'panel.certificates.index', 'panel.databases.index',
            'panel.backups.index', 'panel.security',
        ] as $route) {
            $this->get(route($route))->assertRedirect(route('login'));
        }
    }

    public function test_an_ordinary_account_cannot_reach_the_panel(): void
    {
        $user = User::factory()->create(['is_admin' => false]);

        $this->actingAs($user)->get('/panel')->assertForbidden();
    }

    public function test_an_administrator_without_a_current_mfa_proof_is_gated(): void
    {
        $admin = $this->confirmedAdmin();

        // No proof at all.
        $this->actingAs($admin)->get('/panel')->assertRedirect(route('login'));

        // Stale proof after the MFA generation changes.
        $admin->forceFill(['mfa_version' => 1])->save();
        $this->actingAs($admin)
            ->withSession(['mfa_verified' => ['id' => $admin->getKey(), 'version' => 0]])
            ->get('/panel')
            ->assertRedirect(route('login'));
    }

    public function test_sensitive_pages_are_accessible_after_mfa_without_reconfirming_password(): void
    {
        $admin = $this->confirmedAdmin();

        $this->actingAs($admin)
            ->withSession($this->mfaProof($admin))
            ->get('/panel/security')
            ->assertOk();
    }

    public function test_privileged_livewire_actions_are_denied_without_authorization(): void
    {
        $application = $this->application(User::factory()->create(['is_admin' => true]));

        Livewire::test(ApplicationOperations::class, ['application' => $application->getKey()])
            ->assertStatus(403);
    }

    public function test_unknown_ids_are_not_trusted(): void
    {
        $admin = $this->confirmedAdmin();
        $session = $this->mfaProof($admin);

        $this->actingAs($admin)->withSession($session)->get('/panel/applications/'.Str::uuid())->assertNotFound();
        $this->actingAs($admin)->withSession($session)->get('/panel/tasks/'.Str::uuid())->assertNotFound();
        $this->actingAs($admin)->withSession($session)->get('/panel/deployments/'.Str::uuid())->assertNotFound();
    }

    public function test_task_progress_output_is_escaped_when_rendered(): void
    {
        $admin = $this->confirmedAdmin();
        $task = $this->task();
        $task->recordEvent('output', TaskEventLevel::Info, '<script>alert(1)</script>');

        $response = $this->actingAs($admin)
            ->withSession($this->mfaProof($admin))
            ->get(route('panel.tasks.show', ['task' => $task->getKey()]));

        $response->assertOk()->assertDontSee('<script>alert(1)</script>', false);
        $this->assertStringContainsString('&lt;script&gt;', $response->getContent());
    }

    public function test_stored_secret_values_are_never_rendered(): void
    {
        $admin = $this->confirmedAdmin();
        $application = $this->application($admin);
        ApplicationSecret::create(['application_id' => $application->getKey(), 'name' => 'APP_KEY', 'value' => 'base64:top-secret-value']);

        $this->actingAs($admin)
            ->withSession($this->mfaProof($admin))
            ->get(route('panel.applications.show', ['application' => $application->getKey()]))
            ->assertOk()
            ->assertSee('APP_KEY')
            ->assertDontSee('base64:top-secret-value');
    }

    public function test_queue_jobs_carry_only_the_task_id(): void
    {
        $taskId = '0f6c5f2a-4c4b-4f1e-9c9b-2f1c8a7d6e55';
        $job = new RunTask($taskId);

        $this->assertSame($taskId, $job->taskId);

        $serialized = serialize($job);
        $this->assertStringContainsString($taskId, $serialized);
        $this->assertStringNotContainsString('password', $serialized);
        $this->assertStringNotContainsString('secret', $serialized);
    }
}
