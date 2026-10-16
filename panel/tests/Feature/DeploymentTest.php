<?php

namespace Tests\Feature;

use App\Actions\Deployments\DeploymentPreflight;
use App\Enums\DeploymentState;
use App\Jobs\RunTask;
use App\Livewire\Panel\ApplicationOperations;
use App\Livewire\Panel\ApplicationShow;
use App\Models\Application;
use App\Models\Deployment;
use App\Models\FakeAgentExecution;
use App\Models\User;
use App\Services\ApplicationService;
use App\Services\DeploymentService;
use App\Services\RecentAuthentication;
use App\Services\TaskService;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Support\Carbon;
use Illuminate\Support\Facades\Queue;
use Illuminate\Support\Str;
use Illuminate\Validation\ValidationException;
use Livewire\Livewire;
use Tests\Support\InteractsWithPanel;
use Tests\TestCase;

class DeploymentTest extends TestCase
{
    use InteractsWithPanel, RefreshDatabase;

    private function admin(): User
    {
        return User::factory()->create(['email' => 'admin@example.com', 'is_admin' => true]);
    }

    private function applications(): ApplicationService
    {
        return app(ApplicationService::class);
    }

    private function deployments(): DeploymentService
    {
        return app(DeploymentService::class);
    }

    private function preflight(): DeploymentPreflight
    {
        return app(DeploymentPreflight::class);
    }

    private function readyApplication(User $admin, array $source = [], array $settings = [], array $overrides = []): Application
    {
        $application = $this->applications()->create($admin, array_merge([
            'name' => 'My App',
            'slug' => 'my-app',
            'provider' => 'laravel',
            'runtime' => 'php',
            'runtime_version' => '8.3',
        ], $overrides));

        $this->applications()->updateSource($admin, $application, array_merge([
            'repository_url' => 'https://github.com/org/repo.git',
            'git_ref' => 'main',
        ], $source));

        $this->applications()->updateConfiguration($admin, $application, array_merge([
            'runtime' => 'php',
            'runtime_version' => '8.3',
            'workers' => 2,
            'scheduler' => true,
            'health_path' => 'up',
            'persistent_paths' => ['storage'],
        ], $settings));

        return $application->refresh();
    }

    private function release(Application $application, string $reference = 'rel-1'): Deployment
    {
        return Deployment::create([
            'application_id' => $application->getKey(),
            'operation' => 'application.deploy',
            'runtime' => 'php',
            'mode' => 'fake',
            'state' => DeploymentState::Succeeded,
            'release_reference' => $reference,
            'idempotency_key' => (string) Str::uuid(),
            'requested_at' => now()->subHour(),
            'finished_at' => now()->subMinutes(30),
        ]);
    }

    public function test_preflight_blocks_missing_source_health_and_capability(): void
    {
        config(['hostnur.agent_mode' => 'fake']);
        $admin = $this->admin();

        $bare = $this->applications()->create($admin, ['name' => 'Bare', 'slug' => 'bare', 'provider' => 'laravel', 'runtime' => 'php']);
        $this->assertFalse($this->preflight()->passes($this->preflight()->deployChecks($bare->refresh())));

        config(['hostnur.agent_mode' => 'unavailable']);
        $ready = $this->readyApplication($admin, ['repository_url' => 'ssh://git@github.com/org/repo.git']);
        $checks = $this->preflight()->deployChecks($ready);
        $failed = collect($this->preflight()->failures($checks))->pluck('key')->all();
        $this->assertContains('credential', $failed);
        $this->assertContains('capability', $failed);
    }

    public function test_preflight_passes_for_a_ready_application_in_fake_mode(): void
    {
        config(['hostnur.agent_mode' => 'fake']);
        $application = $this->readyApplication($this->admin());

        $this->assertTrue($this->preflight()->passes($this->preflight()->deployChecks($application)));
    }

    public function test_deploy_requests_one_task_and_is_idempotent(): void
    {
        config(['hostnur.agent_mode' => 'fake']);
        Queue::fake();
        $admin = $this->admin();
        app(RecentAuthentication::class)->confirm($admin);
        $application = $this->readyApplication($admin);

        $key = (string) Str::uuid();
        $first = $this->deployments()->deploy($admin, $application, $key, false);
        $second = $this->deployments()->deploy($admin, $application, $key, false);

        $this->assertSame($first->getKey(), $second->getKey());
        $this->assertDatabaseCount('deployments', 1);
        $this->assertDatabaseCount('tasks', 1);
        $this->assertNotNull($first->task_id);
        Queue::assertPushed(RunTask::class, 1);
    }

    public function test_edited_intent_invalidates_confirmation_in_the_wizard(): void
    {
        config(['hostnur.agent_mode' => 'fake']);
        Queue::fake();
        $admin = $this->confirmedAdmin();
        app(RecentAuthentication::class)->confirm($admin);
        $application = $this->readyApplication($admin);

        $component = Livewire::actingAs($admin)->test(ApplicationOperations::class, ['application' => $application->getKey()]);

        $component->call('confirmPlan')->assertSet('confirmedFingerprint', fn ($value) => $value !== null);

        $component->set('run_migrations', true)->assertSet('confirmedFingerprint', null);

        $component->call('deploy')->assertHasErrors('confirm');
    }

    public function test_running_migrations_requires_a_recovery_acknowledgement(): void
    {
        config(['hostnur.agent_mode' => 'fake']);
        Queue::fake();
        $admin = $this->confirmedAdmin();
        app(RecentAuthentication::class)->confirm($admin);
        $application = $this->readyApplication($admin);

        try {
            $this->deployments()->deploy($admin, $application, (string) Str::uuid(), true, false);
            $this->fail('Expected migrations without an acknowledgement to be refused.');
        } catch (ValidationException $exception) {
            $this->assertArrayHasKey('run_migrations', $exception->errors());
        }

        $deployment = $this->deployments()->deploy($admin, $application, (string) Str::uuid(), true, true);
        $this->assertNotNull($deployment->getKey());
    }

    public function test_a_confirmed_wizard_requests_a_deployment(): void
    {
        config(['hostnur.agent_mode' => 'fake']);
        Queue::fake();
        $admin = $this->confirmedAdmin();
        app(RecentAuthentication::class)->confirm($admin);
        $application = $this->readyApplication($admin);

        Livewire::actingAs($admin)
            ->test(ApplicationOperations::class, ['application' => $application->getKey()])
            ->call('confirmPlan')
            ->call('deploy')
            ->assertHasNoErrors();

        $this->assertDatabaseCount('deployments', 1);
        $this->assertDatabaseCount('tasks', 1);
    }

    public function test_the_application_page_can_request_start_and_stop(): void
    {
        config(['hostnur.agent_mode' => 'fake']);
        Queue::fake();
        $admin = $this->confirmedAdmin();
        app(RecentAuthentication::class)->confirm($admin);
        $application = $this->readyApplication($admin);
        $this->actingAs($admin)->withSession($this->mfaProof($admin) + $this->recentAuth($admin));

        Livewire::test(ApplicationShow::class, ['application' => $application->getKey()])
            ->call('stop')
            ->assertHasNoErrors();

        $this->assertDatabaseHas('tasks', ['operation' => 'application.stop']);

        Livewire::test(ApplicationShow::class, ['application' => $application->getKey()])
            ->call('start')
            ->assertHasNoErrors();

        $this->assertDatabaseHas('tasks', ['operation' => 'application.start']);
    }

    public function test_deployment_state_tracks_the_task_and_stays_labelled_simulated(): void
    {
        config(['hostnur.agent_mode' => 'fake']);
        Queue::fake();
        $admin = $this->admin();
        app(RecentAuthentication::class)->confirm($admin);
        $application = $this->readyApplication($admin);

        $this->travelTo(Carbon::parse('2026-10-13T05:00:00Z'));
        $deployment = $this->deployments()->deploy($admin, $application, (string) Str::uuid(), false);
        $tasks = app(TaskService::class);

        $tasks->run($deployment->task_id);
        $this->travel(2)->seconds();
        $tasks->run($deployment->task_id);

        $deployment = $this->deployments()->refresh($deployment->refresh());

        $this->assertSame(DeploymentState::Succeeded, $deployment->state);
        $this->assertTrue($deployment->isSimulated());
        $this->assertNotNull($deployment->release_reference);
        $this->assertNull($application->refresh()->observed_state);
    }

    public function test_an_unknown_outcome_is_not_success(): void
    {
        config(['hostnur.agent_mode' => 'fake']);
        Queue::fake();
        $admin = $this->admin();
        app(RecentAuthentication::class)->confirm($admin);
        $application = $this->readyApplication($admin);

        $deployment = $this->deployments()->deploy($admin, $application, (string) Str::uuid(), false);
        $tasks = app(TaskService::class);
        $tasks->run($deployment->task_id);

        FakeAgentExecution::query()->delete();
        $tasks->run($deployment->task_id);

        $deployment = $this->deployments()->refresh($deployment->refresh());

        $this->assertSame(DeploymentState::Unknown, $deployment->state);
        $this->assertSame('execution_unknown', $deployment->error_code);
    }

    public function test_restart_and_rollback_create_tasks_and_validate_releases(): void
    {
        config(['hostnur.agent_mode' => 'fake']);
        Queue::fake();
        $admin = $this->admin();
        app(RecentAuthentication::class)->confirm($admin);
        $application = $this->readyApplication($admin);
        $release = $this->release($application);

        $restart = $this->deployments()->restart($admin, $application, (string) Str::uuid());
        $this->assertSame('application.restart', $restart->operation);
        $this->assertNotNull($restart->task_id);

        $rollback = $this->deployments()->rollback($admin, $application, $release->getKey(), (string) Str::uuid());
        $this->assertSame('application.rollback', $rollback->operation);
        $this->assertSame('rel-1', $rollback->previous_release_reference);

        $other = $this->readyApplication($admin, [], [], ['slug' => 'other-app', 'name' => 'Other']);
        $foreign = $this->release($other, 'rel-2');

        try {
            $this->deployments()->rollback($admin, $application, $foreign->getKey(), (string) Str::uuid());
            $this->fail('Expected a foreign release to be rejected.');
        } catch (ValidationException) {
            $this->assertTrue(true);
        }

        try {
            $this->deployments()->rollback($admin, $application, (string) Str::uuid(), (string) Str::uuid());
            $this->fail('Expected an unknown release to be rejected.');
        } catch (ValidationException) {
            $this->assertTrue(true);
        }
    }

    public function test_revoked_authorization_stops_deployment_execution(): void
    {
        config(['hostnur.agent_mode' => 'fake']);
        Queue::fake();
        $admin = $this->admin();
        app(RecentAuthentication::class)->confirm($admin);
        $application = $this->readyApplication($admin);

        $deployment = $this->deployments()->deploy($admin, $application, (string) Str::uuid(), false);

        $admin->forceFill(['is_admin' => false])->save();

        app(TaskService::class)->run($deployment->task_id);
        $deployment = $this->deployments()->refresh($deployment->refresh());

        $this->assertSame(DeploymentState::Failed, $deployment->state);
        $this->assertSame('permission_denied', $deployment->error_code);
    }

    public function test_operations_page_is_gated_and_shows_policy_and_desired_settings(): void
    {
        config(['hostnur.agent_mode' => 'fake']);
        $admin = $this->confirmedAdmin();
        $application = $this->readyApplication($admin);

        $this->get(route('panel.applications.operations', ['application' => $application->getKey()]))
            ->assertRedirect(route('login'));

        $this->actingAs($admin)
            ->withSession(array_merge($this->mfaProof($admin), $this->recentAuth($admin)))
            ->get(route('panel.applications.operations', ['application' => $application->getKey()]))
            ->assertOk()
            ->assertSee('Does not revert the database or uploads.')
            ->assertSee('Run database migrations')
            ->assertSee('Workers / scheduler');
    }
}
