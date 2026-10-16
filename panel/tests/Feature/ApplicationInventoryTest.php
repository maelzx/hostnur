<?php

namespace Tests\Feature;

use App\Enums\AuditAction;
use App\Enums\DeploymentState;
use App\Enums\DomainHttpsPolicy;
use App\Enums\SourceStatus;
use App\Enums\TaskStatus;
use App\Exceptions\ApplicationConflictException;
use App\Livewire\Panel\ApplicationShow;
use App\Models\Application;
use App\Models\ApplicationRevision;
use App\Models\Deployment;
use App\Models\Task;
use App\Models\User;
use App\Services\ApplicationService;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Gate;
use Illuminate\Support\Str;
use Illuminate\Validation\ValidationException;
use Livewire\Livewire;
use Tests\Support\InteractsWithPanel;
use Tests\TestCase;

class ApplicationInventoryTest extends TestCase
{
    use InteractsWithPanel, RefreshDatabase;

    private function admin(): User
    {
        return User::factory()->create(['email' => 'admin@example.com', 'is_admin' => true]);
    }

    private function service(): ApplicationService
    {
        return app(ApplicationService::class);
    }

    private function application(User $admin, array $overrides = []): Application
    {
        return $this->service()->create($admin, array_merge([
            'name' => 'My App',
            'slug' => 'my-app',
            'provider' => 'laravel',
            'runtime' => 'php',
            'runtime_version' => '8.3',
        ], $overrides));
    }

    public function test_an_application_is_created_with_an_immutable_slug_and_initial_revision(): void
    {
        $admin = $this->admin();

        $application = $this->application($admin);

        $this->assertSame('my-app', $application->slug);
        $this->assertSame(1, $application->current_revision);
        $this->assertSame(SourceStatus::NotChecked, $application->source_status);
        $this->assertDatabaseHas('application_revisions', ['application_id' => $application->getKey(), 'revision' => 1]);
        $this->assertDatabaseHas('audit_events', ['action' => AuditAction::ApplicationCreated->value]);
    }

    public function test_duplicate_slugs_and_unknown_providers_are_rejected(): void
    {
        $admin = $this->admin();
        $this->application($admin);

        try {
            $this->application($admin, ['slug' => 'my-app', 'name' => 'Other']);
            $this->fail('Expected duplicate slug to be rejected.');
        } catch (ValidationException $exception) {
            $this->assertArrayHasKey('slug', $exception->errors());
        }

        $this->expectException(ValidationException::class);
        $this->application($admin, ['slug' => 'second', 'provider' => 'untrusted']);
    }

    public function test_application_policy_denies_non_administrators(): void
    {
        $user = User::factory()->create(['is_admin' => false]);

        $this->assertTrue(Gate::forUser($this->admin())->allows('create', Application::class));
        $this->assertTrue(Gate::forUser($user)->denies('create', Application::class));
    }

    public function test_configuration_changes_create_immutable_revisions(): void
    {
        $admin = $this->admin();
        $application = $this->application($admin);

        $revision = $this->service()->updateConfiguration($admin, $application, [
            'runtime' => 'php',
            'runtime_version' => '8.3',
            'build_command' => 'composer install',
            'start_command' => null,
            'workers' => 2,
            'scheduler' => true,
            'health_path' => 'up',
            'persistent_paths' => ['storage', 'public/uploads'],
        ]);

        $this->assertSame(2, $revision->revision);
        $application->refresh();
        $this->assertSame(2, $application->current_revision);

        $first = ApplicationRevision::query()->where('application_id', $application->getKey())->where('revision', 1)->first();
        $this->assertNull($first->settings['build_command']);
        $this->assertSame('composer install', $revision->fresh()->settings['build_command']);
    }

    public function test_an_active_task_revision_is_not_mutated_by_new_configuration(): void
    {
        $admin = $this->admin();
        $application = $this->application($admin);

        $task = Task::create([
            'actor_id' => $admin->getKey(),
            'actor_label' => $admin->email,
            'operation' => 'application.deploy',
            'resource_kind' => 'application',
            'resource_id' => $application->getKey(),
            'config_revision' => '1',
            'mode' => 'fake',
            'status' => TaskStatus::Running,
            'idempotency_key' => (string) Str::uuid(),
            'parameters' => [],
            'payload_fingerprint' => str_repeat('a', 64),
            'requested_at' => now(),
        ]);

        $this->service()->updateConfiguration($admin, $application, [
            'runtime' => 'php',
            'runtime_version' => '8.4',
            'persistent_paths' => [],
        ]);

        $this->assertSame('1', $task->refresh()->config_revision);
        $this->assertSame('8.3', ApplicationRevision::query()->where('revision', 1)->first()->settings['runtime_version']);
    }

    public function test_invalid_configuration_is_rejected(): void
    {
        $admin = $this->admin();
        $application = $this->application($admin);

        foreach ([
            ['runtime' => 'php', 'unknown' => true],
            ['runtime' => 'ruby'],
            ['runtime' => 'php', 'health_path' => '/etc/passwd'],
            ['runtime' => 'php', 'workers' => 11],
            ['runtime' => 'php', 'persistent_paths' => ['../secret']],
        ] as $settings) {
            try {
                $this->service()->updateConfiguration($admin, $application, $settings);
                $this->fail('Expected invalid configuration to be rejected: '.json_encode($settings));
            } catch (ValidationException) {
                $this->assertTrue(true);
            }
        }
    }

    public function test_source_urls_are_normalized_and_marked_not_checked(): void
    {
        $admin = $this->admin();
        $application = $this->application($admin);

        $this->service()->updateSource($admin, $application, [
            'repository_url' => 'https://github.com/Org/Repo.git',
            'git_ref' => 'main',
        ]);

        $application->refresh();
        $this->assertSame('https://github.com/Org/Repo.git', $application->repository_url);
        $this->assertSame(SourceStatus::NotChecked, $application->source_status);

        $this->expectException(ValidationException::class);
        $this->service()->updateSource($admin, $application, ['repository_url' => 'file:///etc/passwd']);
    }

    public function test_the_source_repository_is_locked_after_a_successful_deployment(): void
    {
        $admin = $this->admin();
        $application = $this->application($admin);
        $this->service()->updateSource($admin, $application, ['repository_url' => 'https://github.com/org/one.git']);

        Deployment::create([
            'application_id' => $application->getKey(),
            'operation' => 'application.deploy',
            'runtime' => $application->runtime,
            'mode' => 'fake',
            'state' => DeploymentState::Succeeded,
            'idempotency_key' => (string) Str::uuid(),
        ]);

        // A different repository is refused once deployed...
        try {
            $this->service()->updateSource($admin, $application->refresh(), ['repository_url' => 'https://github.com/org/two.git']);
            $this->fail('Expected the locked repository to be refused.');
        } catch (ValidationException $exception) {
            $this->assertArrayHasKey('repository_url', $exception->errors());
        }

        // ...but the Git ref and credential remain editable.
        $this->service()->updateSource($admin, $application->refresh(), [
            'repository_url' => 'https://github.com/org/one.git',
            'git_ref' => 'release',
        ]);

        $this->assertSame('release', $application->refresh()->git_ref);
    }

    public function test_the_application_page_reads_application_logs_in_fake_mode(): void
    {
        config(['hostnur.agent_mode' => 'fake']);
        $admin = $this->confirmedAdmin();
        $application = $this->application($admin);

        $this->actingAs($admin)->withSession($this->mfaProof($admin) + $this->recentAuth($admin));

        Livewire::test(ApplicationShow::class, ['application' => $application->getKey()])
            ->assertSet('logsUnavailable', null)
            ->assertSet('logLines', fn (mixed $lines): bool => is_array($lines) && $lines !== [])
            ->assertSet('observedLifecycle', 'ready');

        $this->assertDatabaseHas('applications', [
            'id' => $application->getKey(),
            'observed_state' => 'ready',
            'observed_mode' => 'fake',
        ]);
    }

    public function test_credentials_are_encrypted_hidden_and_rotatable(): void
    {
        $admin = $this->admin();
        $credential = $this->service()->storeCredential($admin, [
            'label' => 'Deploy token',
            'type' => 'https_token',
            'secret' => 'plain-token-value',
        ]);

        $raw = DB::table('source_credentials')->where('id', $credential->getKey())->value('secret');
        $this->assertNotSame('plain-token-value', $raw);
        $this->assertArrayNotHasKey('secret', $credential->toArray());
        $this->assertStringNotContainsString('plain-token-value', json_encode($credential));

        $this->service()->rotateCredential($admin, $credential, 'rotated-token-value');
        $this->assertSame(2, $credential->refresh()->version);
        $this->assertNotSame($raw, DB::table('source_credentials')->where('id', $credential->getKey())->value('secret'));
        $this->assertDatabaseHas('audit_events', ['action' => AuditAction::SourceCredentialRotated->value]);
    }

    public function test_secrets_are_write_only_encrypted_and_replaceable(): void
    {
        $admin = $this->admin();
        $application = $this->application($admin);

        $secret = $this->service()->setSecret($admin, $application, 'APP_KEY', 'base64:super-secret');
        $this->assertSame(1, $secret->version);

        $raw = DB::table('application_secrets')->where('name', 'APP_KEY')->value('value');
        $this->assertNotSame('base64:super-secret', $raw);
        $this->assertArrayNotHasKey('value', $secret->toArray());
        $this->assertStringNotContainsString('base64:super-secret', json_encode($secret->fresh()));

        $metadata = (string) DB::table('audit_events')->where('action', AuditAction::ApplicationSecretSet->value)->value('metadata');
        $this->assertStringNotContainsString('base64:super-secret', $metadata);

        $replaced = $this->service()->setSecret($admin, $application, 'APP_KEY', 'base64:rotated');
        $this->assertSame(2, $replaced->version);

        $this->service()->deleteSecret($admin, $application, 'APP_KEY');
        $this->assertDatabaseMissing('application_secrets', ['application_id' => $application->getKey(), 'name' => 'APP_KEY']);
    }

    public function test_a_failed_secret_write_preserves_the_old_value(): void
    {
        $admin = $this->admin();
        $application = $this->application($admin);
        $this->service()->setSecret($admin, $application, 'API_TOKEN', 'original-value');

        try {
            $this->service()->setSecret($admin, $application, 'API_TOKEN', str_repeat('x', 65537));
            $this->fail('Expected oversized secret to be rejected.');
        } catch (ValidationException) {
            $this->assertTrue(true);
        }

        $secret = $application->secrets()->where('name', 'API_TOKEN')->sole();
        $this->assertSame('original-value', $secret->value);
        $this->assertSame(1, $secret->version);
    }

    public function test_domains_are_normalized_and_conflicts_rejected(): void
    {
        $admin = $this->admin();
        $application = $this->application($admin);

        $domain = $this->service()->addDomain($admin, $application, 'App.Example.com.', DomainHttpsPolicy::Auto);
        $this->assertSame('app.example.com', $domain->hostname);

        $other = $this->application($admin, ['slug' => 'other-app', 'name' => 'Other']);

        try {
            $this->service()->addDomain($admin, $other, 'APP.EXAMPLE.COM', DomainHttpsPolicy::Required);
            $this->fail('Expected duplicate hostname to be rejected.');
        } catch (ValidationException) {
            $this->assertTrue(true);
        }

        try {
            $this->service()->addDomain($admin, $application, '*.example.com', DomainHttpsPolicy::Auto);
            $this->fail('Expected wildcard hostname to be rejected.');
        } catch (ValidationException) {
            $this->assertTrue(true);
        }
    }

    public function test_archiving_is_refused_while_work_is_active_and_preserves_history(): void
    {
        $admin = $this->admin();
        $application = $this->application($admin);
        $this->service()->setSecret($admin, $application, 'APP_KEY', 'value');
        $this->service()->addDomain($admin, $application, 'app.example.com', DomainHttpsPolicy::Auto);

        Task::create([
            'actor_id' => $admin->getKey(),
            'actor_label' => $admin->email,
            'operation' => 'application.deploy',
            'resource_kind' => 'application',
            'resource_id' => $application->getKey(),
            'mode' => 'fake',
            'status' => TaskStatus::Queued,
            'idempotency_key' => (string) Str::uuid(),
            'parameters' => [],
            'payload_fingerprint' => str_repeat('b', 64),
            'requested_at' => now(),
        ]);

        try {
            $this->service()->archive($admin, $application);
            $this->fail('Expected archive to be refused while work is active.');
        } catch (ApplicationConflictException) {
            $this->assertNull($application->refresh()->archived_at);
        }

        Task::query()->update(['status' => TaskStatus::Succeeded->value]);
        $this->service()->archive($admin, $application->refresh());

        $this->assertNotNull($application->refresh()->archived_at);
        $this->assertDatabaseHas('application_revisions', ['application_id' => $application->getKey()]);
        $this->assertDatabaseHas('application_secrets', ['application_id' => $application->getKey()]);
        $this->assertDatabaseHas('application_domains', ['application_id' => $application->getKey()]);
    }

    public function test_application_pages_are_gated_and_ids_are_not_trusted(): void
    {
        $admin = $this->confirmedAdmin();
        $application = $this->application($admin);

        $this->get('/panel/applications')->assertRedirect(route('login'));

        $this->actingAs($admin)
            ->withSession($this->mfaProof($admin))
            ->get('/panel/applications')
            ->assertOk()
            ->assertSee('My App');

        $this->actingAs($admin)
            ->withSession($this->mfaProof($admin))
            ->get('/panel/applications/'.$application->getKey())
            ->assertOk()
            ->assertSee('my-app');

        $this->actingAs($admin)
            ->withSession($this->mfaProof($admin))
            ->get('/panel/applications/'.(string) Str::uuid())
            ->assertNotFound();
    }

    public function test_stored_values_are_rendered_escaped(): void
    {
        $admin = $this->confirmedAdmin();
        $application = $this->application($admin, ['name' => '<script>alert(1)</script>']);

        $response = $this->actingAs($admin)
            ->withSession($this->mfaProof($admin))
            ->get('/panel/applications');

        $response->assertOk()->assertDontSee('<script>alert(1)</script>', false);
        $this->assertStringContainsString('&lt;script&gt;', $response->getContent());
    }

    public function test_the_livewire_component_saves_source_and_configuration(): void
    {
        $admin = $this->confirmedAdmin();
        $this->actingAs($admin)->withSession($this->mfaProof($admin) + $this->recentAuth($admin));

        $application = $this->application($admin);

        Livewire::test(ApplicationShow::class, ['application' => $application->getKey()])
            ->set('repository_url', 'https://github.com/maelzx/AssetKOM.git')
            ->set('git_ref', 'main')
            ->call('saveSource')
            ->assertHasNoErrors();

        $this->assertSame(
            'https://github.com/maelzx/AssetKOM.git',
            $application->refresh()->repository_url,
        );

        Livewire::test(ApplicationShow::class, ['application' => $application->getKey()])
            ->set('runtime', 'php')
            ->set('workers', 2)
            ->call('saveConfiguration')
            ->assertHasNoErrors();

        $this->assertSame(2, (int) $application->refresh()->revisions()->first()->settings['workers']);

        Livewire::test(ApplicationShow::class, ['application' => $application->getKey()])
            ->set('workers', 0)
            ->set('runtime_version', '')
            ->call('applyLaravelDefaults')
            ->assertSet('runtime', 'php')
            ->assertSet('health_path', 'up')
            ->assertSet('workers', 1)
            ->assertSet('scheduler', true)
            ->assertSet('persistent_paths_text', 'storage/app');
    }
}
