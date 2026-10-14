<?php

namespace Database\Seeders;

use App\Enums\BackupScope;
use App\Enums\CertificateSource;
use App\Enums\DatabaseEngine;
use App\Enums\DeploymentState;
use App\Enums\DomainHttpsPolicy;
use App\Enums\RequestState;
use App\Enums\TaskEventLevel;
use App\Enums\TaskStatus;
use App\Models\Application;
use App\Models\AuditEvent;
use App\Models\Backup;
use App\Models\BackupPolicy;
use App\Models\Certificate;
use App\Models\DatabaseRequest;
use App\Models\Deployment;
use App\Models\ScheduledJob;
use App\Models\Task;
use App\Models\User;
use App\Services\ApplicationService;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Seeder;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Str;

/**
 * Local demo data for UI/UX work. Run with:
 *   php artisan migrate:fresh --seed
 * or:
 *   php artisan db:seed --class=Database\\Seeders\\DemoSeeder
 */
class DemoSeeder extends Seeder
{
    public function run(): void
    {
        Model::unguarded(function (): void {
            DB::table('panel_setup')->updateOrInsert(
                ['id' => 1],
                ['administrator_bootstrapped_at' => now()],
            );

            $admin = $this->administrator();

            $applications = $this->applications($admin);
            $this->tasks($admin, $applications);
            $this->audit($admin, $applications);
            $this->deployments($admin, $applications);
            $this->databases($admin, $applications);
            $this->certificates($applications);
            $this->schedules($applications);
            $this->backups($admin, $applications);
        });

        $this->command?->info('Demo data seeded. Login: admin@local.test / password123');
    }

    private function administrator(): User
    {
        $admin = User::updateOrCreate(
            ['email' => 'admin@local.test'],
            [
                'name' => 'Demo Admin',
                'password' => 'password123',
                'is_admin' => true,
                'email_verified_at' => now(),
                'mfa_version' => 0,
            ],
        );

        // MFA is intentionally left unenrolled: the first login walks through the
        // panel's MFA enrollment screen (scan the QR with an authenticator app).
        return $admin->refresh();
    }

    /**
     * @return list<Application>
     */
    private function applications(User $admin): array
    {
        $service = app(ApplicationService::class);

        $definitions = [
            [
                'create' => ['name' => 'AssetKOM', 'slug' => 'assetkom', 'provider' => 'laravel', 'runtime' => 'php', 'runtime_version' => '8.4'],
                'repository' => 'https://github.com/maelzx/AssetKOM.git',
                'domain' => 'assetkom.example.com',
                'secrets' => ['APP_KEY' => 'base64:demoassetkey0000000000000000000000=', 'MAIL_MAILER' => 'smtp'],
            ],
            [
                'create' => ['name' => 'BookingKOM', 'slug' => 'bookingkom', 'provider' => 'laravel', 'runtime' => 'php', 'runtime_version' => '8.3'],
                'repository' => 'https://github.com/maelzx/BookingKOM.git',
                'domain' => 'booking.example.com',
                'secrets' => ['APP_KEY' => 'base64:demobookingkey00000000000000000000='],
            ],
            [
                'create' => ['name' => 'Marketing Site', 'slug' => 'marketing-site', 'provider' => 'laravel', 'runtime' => 'static', 'runtime_version' => null],
                'repository' => 'https://github.com/example/marketing-site.git',
                'domain' => 'www.example.com',
                'secrets' => [],
            ],
        ];

        $applications = [];

        foreach ($definitions as $definition) {
            $application = $service->create($admin, $definition['create']);
            $service->updateSource($admin, $application, [
                'repository_url' => $definition['repository'],
                'git_ref' => 'main',
                'source_credential_id' => null,
            ]);
            $service->addDomain($admin, $application, $definition['domain'], DomainHttpsPolicy::Auto);

            foreach ($definition['secrets'] as $name => $value) {
                $service->setSecret($admin, $application, $name, $value);
            }

            $applications[] = $application->refresh();
        }

        return $applications;
    }

    /**
     * @param  list<Application>  $applications
     */
    private function tasks(User $admin, array $applications): void
    {
        $specs = [
            ['application.deploy', TaskStatus::Succeeded, 'application', $applications[0]->getKey()],
            ['application.deploy', TaskStatus::Failed, 'application', $applications[1]->getKey()],
            ['application.restart', TaskStatus::Running, 'application', $applications[0]->getKey()],
            ['server.snapshot', TaskStatus::Succeeded, null, null],
            ['package.install', TaskStatus::AwaitingApproval, null, null],
            ['certificate.issue', TaskStatus::Succeeded, 'domain', $applications[0]->getKey()],
            ['database.provision', TaskStatus::Queued, null, null],
            ['application.rollback', TaskStatus::Cancelled, 'application', $applications[2]->getKey()],
        ];

        foreach ($specs as $index => [$operation, $status, $kind, $id]) {
            $task = Task::create([
                'actor_id' => $admin->getKey(),
                'actor_label' => $admin->email,
                'operation' => $operation,
                'resource_kind' => $kind,
                'resource_id' => $id,
                'config_revision' => '1',
                'mode' => 'fake',
                'status' => $status,
                'idempotency_key' => (string) Str::uuid(),
                'parameters' => [],
                'payload_fingerprint' => hash('sha256', $operation.$index),
                'requested_at' => now()->subMinutes(30 - $index * 3),
            ]);

            $task->recordEvent('requested', TaskEventLevel::Info, 'Requested '.$operation.'.');
            $task->recordEvent('approved', TaskEventLevel::Info, 'Approved by '.$admin->email.'.');

            if ($status === TaskStatus::Running) {
                $task->recordEvent('progress', TaskEventLevel::Info, 'Running… 40% complete.');
            } elseif ($status === TaskStatus::Succeeded) {
                $task->recordEvent('output', TaskEventLevel::Info, 'Completed successfully in 2.4s.');
            } elseif ($status === TaskStatus::Failed) {
                $task->recordEvent('error', TaskEventLevel::Error, 'Operation failed: dependency step exited with code 1.');
            }
        }
    }

    /**
     * @param  list<Application>  $applications
     */
    private function audit(User $admin, array $applications): void
    {
        $events = [
            ['auth.login', 'success', null, null],
            ['application.created', 'success', 'application', $applications[0]->getKey()],
            ['application.source_updated', 'success', 'application', $applications[0]->getKey()],
            ['application.deploy_requested', 'requested', 'application', $applications[0]->getKey()],
            ['application.deploy_requested', 'failure', 'application', $applications[1]->getKey()],
            ['server.package_install_requested', 'requested', null, null],
            ['auth.mfa_verified', 'success', null, null],
        ];

        foreach ($events as $index => [$action, $outcome, $kind, $id]) {
            AuditEvent::create([
                'actor_id' => $admin->getKey(),
                'actor_label' => $admin->email,
                'action' => $action,
                'outcome' => $outcome,
                'resource_kind' => $kind,
                'resource_id' => $id,
                'metadata' => ['operation' => $action],
                'recorded_at' => now()->subMinutes(15 - $index),
            ]);
        }
    }

    /**
     * @param  list<Application>  $applications
     */
    private function deployments(User $admin, array $applications): void
    {
        $specs = [
            [$applications[0], DeploymentState::Succeeded, '8f3c1a2b', 'main'],
            [$applications[0], DeploymentState::Succeeded, 'b7d9e4f1', 'main'],
            [$applications[1], DeploymentState::Failed, '1c2d3e4f', 'develop'],
            [$applications[1], DeploymentState::Running, '9a8b7c6d', 'main'],
        ];

        foreach ($specs as $index => [$application, $state, $commit, $ref]) {
            Deployment::create([
                'application_id' => $application->getKey(),
                'revision_number' => 1,
                'operation' => 'application.deploy',
                'git_ref' => $ref,
                'resolved_commit' => str_pad($commit, 40, '0'),
                'runtime' => $application->runtime,
                'mode' => 'fake',
                'state' => $state,
                'run_migrations' => $index === 0,
                'idempotency_key' => (string) Str::uuid(),
                'requested_at' => now()->subMinutes(20 - $index * 4),
                'created_by' => $admin->getKey(),
            ]);
        }
    }

    /**
     * @param  list<Application>  $applications
     */
    private function databases(User $admin, array $applications): void
    {
        DatabaseRequest::create([
            'application_id' => $applications[0]->getKey(),
            'engine' => DatabaseEngine::MariaDb,
            'database_name' => 'hn_db_assetkom',
            'username' => 'hn_u_assetkom',
            'password' => Str::password(32, symbols: false),
            'state' => RequestState::Succeeded,
        ]);

        DatabaseRequest::create([
            'application_id' => $applications[1]->getKey(),
            'engine' => DatabaseEngine::MariaDb,
            'database_name' => 'hn_db_booking',
            'username' => 'hn_u_booking',
            'password' => Str::password(32, symbols: false),
            'state' => RequestState::Requested,
        ]);
    }

    /**
     * @param  list<Application>  $applications
     */
    private function certificates(array $applications): void
    {
        foreach ($applications as $application) {
            $domain = $application->domains()->first();

            if ($domain === null) {
                continue;
            }

            Certificate::create([
                'application_domain_id' => $domain->getKey(),
                'hostname' => $domain->hostname,
                'source' => CertificateSource::ManagedHttp01,
                'provider' => 'http01',
                'state' => RequestState::Succeeded,
                'issued_at' => now()->subDays(5),
                'expires_at' => now()->addDays(85),
            ]);
        }
    }

    /**
     * @param  list<Application>  $applications
     */
    private function schedules(array $applications): void
    {
        ScheduledJob::create([
            'application_id' => $applications[0]->getKey(),
            'name' => 'Daily alert digest',
            'expression' => '0 8 * * *',
            'timezone' => 'Asia/Kuala_Lumpur',
            'command_reference' => 'artisan alerts:digest',
            'enabled' => true,
            'revision' => 1,
        ]);

        ScheduledJob::create([
            'application_id' => $applications[0]->getKey(),
            'name' => 'Nightly depreciation',
            'expression' => '30 2 * * *',
            'timezone' => 'Asia/Kuala_Lumpur',
            'command_reference' => 'artisan assets:depreciate',
            'enabled' => false,
            'revision' => 1,
        ]);
    }

    /**
     * @param  list<Application>  $applications
     */
    private function backups(User $admin, array $applications): void
    {
        $policy = BackupPolicy::create([
            'application_id' => $applications[0]->getKey(),
            'scope' => BackupScope::Application,
            'expression' => '0 3 * * *',
            'timezone' => 'Asia/Kuala_Lumpur',
            'retention_days' => 14,
            'enabled' => true,
        ]);

        Backup::create([
            'backup_policy_id' => $policy->getKey(),
            'application_id' => $applications[0]->getKey(),
            'mode' => 'fake',
            'state' => RequestState::Succeeded,
            'verification_status' => 'unverified',
            'size_bytes' => 5_242_880,
        ]);

        Backup::create([
            'backup_policy_id' => $policy->getKey(),
            'application_id' => $applications[0]->getKey(),
            'mode' => 'fake',
            'state' => RequestState::Failed,
            'verification_status' => 'unverified',
            'size_bytes' => null,
        ]);
    }
}
