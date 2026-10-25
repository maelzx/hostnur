<?php

namespace App\Actions\Applications;

use App\Enums\DomainHttpsPolicy;
use App\Models\Application;
use App\Models\User;
use App\Services\ApplicationService;
use App\Services\CertificateService;
use App\Services\DatabaseService;
use App\Services\ScheduleService;
use Illuminate\Support\Facades\DB;

/**
 * Create an application with its source, configuration, domain, certificate
 * request, database request, secrets and schedule in one transaction, so a
 * mid-way failure never leaves a half-built application (which would also
 * dead-end retries on "slug already in use"). Shared by the create wizard and
 * the `hostnur:smoke` command so both exercise the same path.
 */
final class CreateApplication
{
    public function __construct(
        private readonly ApplicationService $applications,
        private readonly CertificateService $certificates,
        private readonly DatabaseService $databases,
        private readonly ScheduleService $schedules,
    ) {}

    /**
     * @param  array{
     *     name: string,
     *     slug: string,
     *     provider: string,
     *     runtime: string,
     *     runtime_version?: string|null,
     *     repository_url?: string|null,
     *     git_ref?: string|null,
     *     source_credential_id?: string|null,
     *     configuration?: array<string, mixed>|null,
     *     hostname?: string|null,
     *     https_policy?: DomainHttpsPolicy,
     *     certificate_key?: string|null,
     *     database?: array{engine: string, name: string, username: string, key: string}|null,
     *     secrets?: array<string, string>,
     *     schedule?: array<string, mixed>|null,
     * }  $input  `certificate_key`/`database`/`schedule` null means "not requested".
     */
    public function handle(User $actor, array $input): Application
    {
        return DB::transaction(function () use ($actor, $input): Application {
            $application = $this->applications->create($actor, [
                'name' => $input['name'],
                'slug' => $input['slug'],
                'provider' => $input['provider'],
                'runtime' => $input['runtime'],
                'runtime_version' => $input['runtime_version'] ?? null,
            ]);

            $repository = trim((string) ($input['repository_url'] ?? ''));

            if ($repository !== '') {
                $this->applications->updateSource($actor, $application, [
                    'repository_url' => $repository,
                    'git_ref' => $input['git_ref'] ?? null,
                    'source_credential_id' => $input['source_credential_id'] ?? null,
                ]);
            }

            if (($input['configuration'] ?? null) !== null) {
                $this->applications->updateConfiguration($actor, $application->refresh(), $input['configuration']);
            }

            $hostname = trim((string) ($input['hostname'] ?? ''));

            if ($hostname !== '') {
                $domain = $this->applications->addDomain(
                    $actor,
                    $application,
                    $hostname,
                    $input['https_policy'] ?? DomainHttpsPolicy::Auto,
                );

                if (($input['certificate_key'] ?? null) !== null) {
                    $this->certificates->requestManaged($actor, $domain, $input['certificate_key']);
                }
            }

            if (($database = $input['database'] ?? null) !== null) {
                $this->databases->request(
                    $actor,
                    $application,
                    $database['engine'],
                    $database['name'],
                    $database['username'],
                    $database['key'],
                );
            }

            foreach ($input['secrets'] ?? [] as $name => $value) {
                $this->applications->setSecret($actor, $application, $name, $value);
            }

            if (($input['schedule'] ?? null) !== null) {
                $this->schedules->save($actor, $application, $input['schedule']);
            }

            return $application;
        });
    }
}
