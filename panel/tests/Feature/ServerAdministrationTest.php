<?php

namespace Tests\Feature;

use App\Enums\BackupScope;
use App\Enums\RequestState;
use App\Enums\ServiceAction;
use App\Models\Application;
use App\Models\ApplicationDomain;
use App\Models\BackupPolicy;
use App\Models\ScheduledJob;
use App\Models\User;
use App\Services\BackupService;
use App\Services\CertificateService;
use App\Services\DatabaseService;
use App\Services\RecentAuthentication;
use App\Services\ScheduleService;
use App\Services\ServerService;
use App\Services\TaskService;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Queue;
use Illuminate\Validation\ValidationException;
use Tests\Support\InteractsWithPanel;
use Tests\TestCase;

class ServerAdministrationTest extends TestCase
{
    use InteractsWithPanel, RefreshDatabase;

    private function admin(): User
    {
        return User::factory()->create(['email' => 'admin@example.com', 'is_admin' => true]);
    }

    private function application(): Application
    {
        return Application::create([
            'name' => 'My App',
            'slug' => 'my-app',
            'provider' => 'laravel',
            'runtime' => 'php',
            'runtime_version' => '8.3',
            'source_status' => 'not_checked',
            'current_revision' => 1,
        ]);
    }

    private function recent(User $admin): void
    {
        app(RecentAuthentication::class)->confirm($admin);
    }

    public function test_the_packages_and_firewall_pages_are_gated_and_show_snapshot(): void
    {
        config(['hostnur.agent_mode' => 'fake']);
        $admin = $this->confirmedAdmin();

        $this->get('/panel/packages')->assertRedirect(route('login'));
        $this->get('/panel/firewall')->assertRedirect(route('login'));

        $session = array_merge($this->mfaProof($admin), $this->recentAuth($admin));

        $this->actingAs($admin)
            ->withSession($session)
            ->get('/panel/packages')
            ->assertOk()
            ->assertSee('nginx')
            ->assertSee('certbot');

        $this->actingAs($admin)
            ->withSession($session)
            ->get('/panel/firewall')
            ->assertOk()
            ->assertSee('Firewall');
    }

    public function test_service_control_requires_a_snapshot_service(): void
    {
        config(['hostnur.agent_mode' => 'fake']);
        Queue::fake();
        $admin = $this->admin();
        $this->recent($admin);

        $service = app(ServerService::class);

        try {
            $service->control($admin, '/etc/passwd', ServiceAction::Restart, 'k1');
            $this->fail('Expected an arbitrary service id to be rejected.');
        } catch (ValidationException) {
            $this->assertTrue(true);
        }

        $task = $service->control($admin, 'nginx', ServiceAction::Restart, 'k2');
        $this->assertNotNull($task->getKey());
        $this->assertDatabaseHas('tasks', ['id' => $task->getKey(), 'operation' => 'service.control']);
    }

    public function test_package_install_requires_a_catalog_package(): void
    {
        config(['hostnur.agent_mode' => 'fake']);
        Queue::fake();
        $admin = $this->admin();
        $this->recent($admin);

        $service = app(ServerService::class);

        try {
            $service->installPackage($admin, 'totally-arbitrary; rm -rf /', 'k1');
            $this->fail('Expected an arbitrary package to be rejected.');
        } catch (ValidationException) {
            $this->assertTrue(true);
        }

        $task = $service->installPackage($admin, 'nginx', 'k2');
        $this->assertDatabaseHas('tasks', ['id' => $task->getKey(), 'operation' => 'package.install']);
    }

    public function test_requests_are_closed_when_the_agent_is_unavailable(): void
    {
        config(['hostnur.agent_mode' => 'unavailable']);
        $admin = $this->admin();
        $this->recent($admin);

        $this->expectException(ValidationException::class);
        app(ServerService::class)->control($admin, 'nginx', ServiceAction::Restart, 'k1');
    }

    public function test_schedules_validate_and_do_not_install_cron(): void
    {
        Queue::fake();
        $admin = $this->admin();
        $this->recent($admin);
        $application = $this->application();
        $service = app(ScheduleService::class);

        $job = $service->save($admin, $application, [
            'name' => 'nightly',
            'expression' => '0 2 * * *',
            'timezone' => 'UTC',
            'command_reference' => 'schedule:run',
        ]);

        $this->assertTrue($job->enabled);
        $this->assertSame('0 2 * * *', $job->expression);
        $this->assertDatabaseHas('audit_events', ['action' => 'application.schedule_created']);

        foreach ([
            ['expression' => 'not a cron', 'timezone' => 'UTC'],
            ['expression' => '0 2 * * *', 'timezone' => 'Mars/Olympus'],
            ['expression' => '0 2 * * *', 'timezone' => 'UTC', 'command_reference' => '-rf'],
        ] as $bad) {
            try {
                $service->save($admin, $application, array_merge([
                    'name' => 'bad-'.uniqid(),
                    'expression' => '0 2 * * *',
                    'timezone' => 'UTC',
                    'command_reference' => 'schedule:run',
                ], $bad));
                $this->fail('Expected invalid schedule to be rejected.');
            } catch (ValidationException) {
                $this->assertTrue(true);
            }
        }

        $task = $service->apply($admin, $job, 'apply-1');
        $this->assertDatabaseHas('tasks', ['id' => $task->getKey(), 'operation' => 'schedule.apply']);
    }

    public function test_certificate_ingestion_validates_with_openssl_and_hides_the_key(): void
    {
        $admin = $this->admin();
        $application = $this->application();
        $domain = ApplicationDomain::create(['application_id' => $application->getKey(), 'hostname' => 'app.example.com', 'https_policy' => 'auto']);

        [$pem, $keyPem] = $this->selfSigned('app.example.com');

        $certificate = app(CertificateService::class)->ingestUserSupplied($admin, $domain, $pem, $keyPem, null);

        $this->assertSame(RequestState::Succeeded, $certificate->state);
        $this->assertArrayNotHasKey('private_key', $certificate->toArray());
        $this->assertStringNotContainsString('PRIVATE KEY', json_encode($certificate));

        $raw = DB::table('certificates')->where('id', $certificate->getKey())->value('private_key');
        $this->assertStringNotContainsString('PRIVATE KEY', (string) $raw);

        try {
            app(CertificateService::class)->ingestUserSupplied($admin, $domain, $pem, $this->selfSigned('other.example.com')[1], null);
            $this->fail('Expected a mismatched key to be rejected.');
        } catch (ValidationException) {
            $this->assertTrue(true);
        }
    }

    public function test_managed_certificate_request_rejects_wildcards_and_creates_a_task(): void
    {
        Queue::fake();
        $admin = $this->admin();
        $this->recent($admin);
        $application = $this->application();
        $domain = ApplicationDomain::create(['application_id' => $application->getKey(), 'hostname' => 'app.example.com', 'https_policy' => 'auto']);
        $wildcard = ApplicationDomain::create(['application_id' => $application->getKey(), 'hostname' => 'wild.example.com', 'https_policy' => 'auto']);
        $wildcard->forceFill(['hostname' => '*.example.com'])->save();

        $service = app(CertificateService::class);

        $certificate = $service->requestManaged($admin, $domain, 'cert-1');
        $this->assertSame(RequestState::Requested, $certificate->state);
        $this->assertDatabaseHas('tasks', ['operation' => 'certificate.issue']);

        try {
            $service->requestManaged($admin, $wildcard, 'cert-2');
            $this->fail('Expected a wildcard hostname to be rejected.');
        } catch (ValidationException) {
            $this->assertTrue(true);
        }
    }

    public function test_database_requests_generate_hidden_credentials(): void
    {
        Queue::fake();
        $admin = $this->admin();
        $this->recent($admin);
        $application = $this->application();
        $service = app(DatabaseService::class);

        $request = $service->request($admin, $application, 'mariadb', 'my_app', 'my_app_user', 'db-1');

        $this->assertSame('mariadb', $request->engine->value);
        $this->assertArrayNotHasKey('password', $request->toArray());
        $raw = DB::table('database_requests')->where('id', $request->getKey())->value('password');
        $this->assertNotSame('my_app_user', $raw);
        $this->assertDatabaseHas('tasks', ['operation' => 'database.provision']);

        try {
            $service->request($admin, $application, 'oracle', 'x', 'y', 'db-2');
            $this->fail('Expected an unsupported engine to be rejected.');
        } catch (ValidationException) {
            $this->assertTrue(true);
        }
    }

    public function test_backup_policies_and_simulated_backups_are_never_recoverable(): void
    {
        config(['hostnur.agent_mode' => 'fake']);
        Queue::fake();
        $admin = $this->admin();
        $this->recent($admin);
        $application = $this->application();
        $service = app(BackupService::class);

        $policy = $service->savePolicy($admin, [
            'scope' => 'application',
            'application_id' => $application->getKey(),
            'expression' => '0 3 * * *',
            'timezone' => 'UTC',
            'retention_days' => 14,
        ]);

        $backup = $service->requestBackup($admin, $policy, 'backup-1');

        $tasks = app(TaskService::class);
        $tasks->run($backup->task_id);
        $this->travel(2)->seconds();
        $tasks->run($backup->task_id);

        $backup = $service->syncState($backup->refresh());

        $this->assertSame(RequestState::Succeeded, $backup->state);
        $this->assertTrue($backup->isSimulated());
        $this->assertFalse($backup->isRecoverable());
        $this->assertSame('simulated', $backup->verification_status);

        try {
            $service->savePolicy($admin, [
                'scope' => 'application',
                'expression' => '0 3 * * *',
                'timezone' => 'UTC',
                'retention_days' => 7,
            ]);
            $this->fail('Expected an application-scoped policy without an application to be rejected.');
        } catch (ValidationException) {
            $this->assertTrue(true);
        }
    }

    public function test_phase8_pages_render_for_an_authorized_administrator(): void
    {
        config(['hostnur.agent_mode' => 'fake']);
        $admin = $this->confirmedAdmin();
        $application = $this->application();

        ScheduledJob::create(['application_id' => $application->getKey(), 'name' => 'nightly', 'expression' => '0 2 * * *', 'timezone' => 'UTC', 'command_reference' => 'schedule:run']);
        BackupPolicy::create(['application_id' => $application->getKey(), 'scope' => BackupScope::Application, 'expression' => '0 3 * * *', 'timezone' => 'UTC', 'retention_days' => 7]);

        $session = array_merge($this->mfaProof($admin), $this->recentAuth($admin));

        $this->get(route('panel.backups.index'))->assertRedirect(route('login'));

        foreach (['panel.schedules.index', 'panel.certificates.index', 'panel.backups.index', 'panel.packages', 'panel.firewall'] as $route) {
            $this->actingAs($admin)->withSession($session)->get(route($route))->assertOk();
        }
    }

    /**
     * @return array{string, string}
     */
    private function selfSigned(string $commonName): array
    {
        $key = openssl_pkey_new(['private_key_bits' => 2048, 'private_key_type' => OPENSSL_KEYTYPE_RSA]);
        $csr = openssl_csr_new(['commonName' => $commonName], $key, ['digest_alg' => 'sha256']);
        $certificate = openssl_csr_sign($csr, null, $key, 365, ['digest_alg' => 'sha256']);

        openssl_x509_export($certificate, $pem);
        openssl_pkey_export($key, $keyPem);

        return [$pem, $keyPem];
    }
}
