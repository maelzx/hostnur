<?php

namespace App\Services;

use App\Enums\AuditAction;
use App\Enums\AuditOutcome;
use App\Enums\DomainHttpsPolicy;
use App\Enums\SourceCredentialType;
use App\Enums\SourceStatus;
use App\Exceptions\ApplicationConflictException;
use App\Models\Application;
use App\Models\ApplicationDomain;
use App\Models\ApplicationRevision;
use App\Models\ApplicationSecret;
use App\Models\SourceCredential;
use App\Models\Task;
use App\Models\User;
use App\Support\GitRepositoryUrl;
use App\Support\Hostname;
use Illuminate\Database\UniqueConstraintViolationException;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Validator;
use Illuminate\Validation\Rule;
use Illuminate\Validation\ValidationException;
use InvalidArgumentException;

/**
 * The single orchestration path for application configuration. Values are
 * hosted-app intent only: they never touch the panel's own environment, agent
 * mode or filesystem, and no network/host call is made on save.
 */
final class ApplicationService
{
    public function __construct(
        private readonly AuditRecorder $audit,
    ) {}

    /**
     * @param  array<string, mixed>  $data
     */
    public function create(User $actor, array $data): Application
    {
        $validated = Validator::make($data, [
            'name' => ['required', 'string', 'max:255'],
            'slug' => ['required', 'string', 'min:3', 'max:63', 'regex:/^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/', 'unique:applications,slug'],
            'provider' => ['required', 'string', Rule::in(config('hostnur.applications.providers'))],
            'runtime' => ['required', 'string', Rule::in(config('hostnur.applications.runtimes'))],
            'runtime_version' => ['nullable', 'string', 'max:32'],
        ])->validate();

        try {
            $application = DB::transaction(function () use ($actor, $validated): Application {
                $application = Application::create([
                    'name' => trim($validated['name']),
                    'slug' => $validated['slug'],
                    'provider' => $validated['provider'],
                    'runtime' => $validated['runtime'],
                    'runtime_version' => $validated['runtime_version'] ?? null,
                    'source_status' => SourceStatus::NotChecked,
                    'current_revision' => 1,
                ]);

                ApplicationRevision::create([
                    'application_id' => $application->getKey(),
                    'revision' => 1,
                    'settings' => $this->normalizedSettings($validated + [
                        'build_command' => null,
                        'start_command' => null,
                        'workers' => 0,
                        'scheduler' => false,
                        'health_path' => null,
                        'persistent_paths' => [],
                    ]),
                    'credential_versions' => [],
                    'created_by' => $actor->getKey(),
                    'created_at' => now(),
                ]);

                return $application;
            });
        } catch (UniqueConstraintViolationException) {
            throw ValidationException::withMessages(['slug' => 'That slug is already in use.']);
        }

        $this->audit->record($actor, AuditAction::ApplicationCreated, AuditOutcome::Success, [
            'resource_kind' => 'application',
            'resource_id' => $application->getKey(),
            'metadata' => ['operation' => 'application.create'],
        ]);

        return $application;
    }

    /**
     * @param  array<string, mixed>  $settings
     */
    public function updateConfiguration(User $actor, Application $application, array $settings): ApplicationRevision
    {
        $validated = $this->normalizedSettings($this->validateSettings($settings));

        $revision = DB::transaction(function () use ($actor, $application, $validated): ApplicationRevision {
            $revision = ApplicationRevision::create([
                'application_id' => $application->getKey(),
                'revision' => $application->current_revision + 1,
                'settings' => $validated,
                'credential_versions' => $this->credentialVersions($application),
                'created_by' => $actor->getKey(),
                'created_at' => now(),
            ]);

            $application->forceFill([
                'runtime' => $validated['runtime'],
                'runtime_version' => $validated['runtime_version'] ?? null,
                'current_revision' => $revision->revision,
            ])->save();

            return $revision;
        });

        $this->audit->record($actor, AuditAction::ApplicationUpdated, AuditOutcome::Success, [
            'resource_kind' => 'application',
            'resource_id' => $application->getKey(),
            'metadata' => ['operation' => 'application.configure'],
        ]);

        return $revision;
    }

    /**
     * @param  array<string, mixed>  $data
     */
    public function updateSource(User $actor, Application $application, array $data): Application
    {
        $repository = $data['repository_url'] ?? null;
        $normalized = null;

        if (is_string($repository) && trim($repository) !== '') {
            try {
                $normalized = GitRepositoryUrl::normalize($repository);
            } catch (InvalidArgumentException $exception) {
                throw ValidationException::withMessages(['repository_url' => $exception->getMessage()]);
            }
        }

        if ($application->hasSuccessfulDeployment() && $normalized !== $application->repository_url) {
            throw ValidationException::withMessages([
                'repository_url' => 'The source repository is locked after the first successful deployment. Create a new application to use a different repository.',
            ]);
        }

        $validated = Validator::make([
            'git_ref' => $data['git_ref'] ?? null,
            'source_credential_id' => $data['source_credential_id'] ?? null,
        ], [
            'git_ref' => ['nullable', 'string', 'max:255', 'regex:/^[A-Za-z0-9][A-Za-z0-9._\/-]*$/'],
            'source_credential_id' => ['nullable', 'uuid', Rule::exists('source_credentials', 'id')],
        ])->validate();

        $application->forceFill([
            'repository_url' => $normalized,
            'git_ref' => $validated['git_ref'] ?? null,
            'source_credential_id' => $validated['source_credential_id'] ?? null,
            'source_status' => SourceStatus::NotChecked,
        ])->save();

        $this->audit->record($actor, AuditAction::ApplicationSourceUpdated, AuditOutcome::Success, [
            'resource_kind' => 'application',
            'resource_id' => $application->getKey(),
            'metadata' => ['operation' => 'application.source'],
        ]);

        return $application;
    }

    /**
     * @param  array<string, mixed>  $data
     */
    public function storeCredential(User $actor, array $data): SourceCredential
    {
        $validated = Validator::make($data, [
            'label' => ['required', 'string', 'max:255'],
            'type' => ['required', Rule::enum(SourceCredentialType::class)],
            'secret' => ['required', 'string', 'max:65536'],
            'public_key' => ['nullable', 'string', 'max:65536'],
            'host_key' => ['nullable', 'string', 'max:65536'],
        ])->validate();

        $credential = SourceCredential::create([
            'label' => $validated['label'],
            'type' => $validated['type'],
            'secret' => $validated['secret'],
            'public_key' => $validated['public_key'] ?? null,
            'host_key' => $validated['host_key'] ?? null,
            'version' => 1,
            'rotated_at' => now(),
        ]);

        $this->audit->record($actor, AuditAction::SourceCredentialStored, AuditOutcome::Success, [
            'resource_kind' => 'source_credential',
            'resource_id' => $credential->getKey(),
            'metadata' => ['operation' => 'credential.store'],
        ]);

        return $credential;
    }

    public function rotateCredential(User $actor, SourceCredential $credential, string $secret): SourceCredential
    {
        if ($secret === '' || strlen($secret) > 65536) {
            throw ValidationException::withMessages(['secret' => 'The credential secret is required and bounded to 64 KiB.']);
        }

        $credential->forceFill([
            'secret' => $secret,
            'version' => $credential->version + 1,
            'rotated_at' => now(),
        ])->save();

        $this->audit->record($actor, AuditAction::SourceCredentialRotated, AuditOutcome::Success, [
            'resource_kind' => 'source_credential',
            'resource_id' => $credential->getKey(),
            'metadata' => ['operation' => 'credential.rotate'],
        ]);

        return $credential;
    }

    public function setSecret(User $actor, Application $application, string $name, string $value): ApplicationSecret
    {
        $validated = Validator::make(['name' => $name, 'value' => $value], [
            'name' => ['required', 'string', 'max:255', 'regex:/^[A-Za-z_][A-Za-z0-9_]*$/'],
            'value' => ['required', 'string', 'max:65536'],
        ])->validate();

        if ($application->secrets()->count() >= (int) config('hostnur.applications.max_secrets', 100)
            && ! $application->secrets()->where('name', $validated['name'])->exists()) {
            throw ValidationException::withMessages(['name' => 'This application has reached its secret limit.']);
        }

        return DB::transaction(function () use ($actor, $application, $validated): ApplicationSecret {
            $secret = $application->secrets()->where('name', $validated['name'])->first();

            if ($secret === null) {
                $secret = $application->secrets()->create([
                    'name' => $validated['name'],
                    'value' => $validated['value'],
                    'version' => 1,
                    'rotated_at' => now(),
                ]);
            } else {
                $secret->forceFill([
                    'value' => $validated['value'],
                    'version' => $secret->version + 1,
                    'rotated_at' => now(),
                ])->save();
            }

            $this->audit->record($actor, AuditAction::ApplicationSecretSet, AuditOutcome::Success, [
                'resource_kind' => 'application',
                'resource_id' => $application->getKey(),
                'metadata' => ['operation' => 'application.secret_set'],
            ]);

            return $secret;
        });
    }

    public function deleteSecret(User $actor, Application $application, string $name): void
    {
        $application->secrets()->where('name', $name)->delete();

        $this->audit->record($actor, AuditAction::ApplicationSecretDeleted, AuditOutcome::Success, [
            'resource_kind' => 'application',
            'resource_id' => $application->getKey(),
            'metadata' => ['operation' => 'application.secret_deleted'],
        ]);
    }

    public function addDomain(User $actor, Application $application, string $hostname, DomainHttpsPolicy $policy): ApplicationDomain
    {
        try {
            $normalized = Hostname::normalize($hostname);
        } catch (InvalidArgumentException $exception) {
            throw ValidationException::withMessages(['hostname' => $exception->getMessage()]);
        }

        if ($application->domains()->count() >= (int) config('hostnur.applications.max_domains', 20)) {
            throw ValidationException::withMessages(['hostname' => 'This application has reached its domain limit.']);
        }

        if (ApplicationDomain::query()->where('hostname', $normalized)->exists()) {
            throw ValidationException::withMessages(['hostname' => 'That hostname is already configured.']);
        }

        try {
            $domain = $application->domains()->create([
                'hostname' => $normalized,
                'https_policy' => $policy,
            ]);
        } catch (UniqueConstraintViolationException) {
            throw ValidationException::withMessages(['hostname' => 'That hostname is already configured.']);
        }

        $this->audit->record($actor, AuditAction::ApplicationDomainAdded, AuditOutcome::Success, [
            'resource_kind' => 'application',
            'resource_id' => $application->getKey(),
            'metadata' => ['operation' => 'application.domain_added'],
        ]);

        return $domain;
    }

    public function removeDomain(User $actor, ApplicationDomain $domain): void
    {
        $applicationId = $domain->application_id;
        $domain->delete();

        $this->audit->record($actor, AuditAction::ApplicationDomainRemoved, AuditOutcome::Success, [
            'resource_kind' => 'application',
            'resource_id' => $applicationId,
            'metadata' => ['operation' => 'application.domain_removed'],
        ]);
    }

    public function archive(User $actor, Application $application): Application
    {
        if ($application->isArchived()) {
            return $application;
        }

        $activeTask = Task::query()
            ->where('resource_kind', 'application')
            ->where('resource_id', $application->getKey())
            ->whereIn('status', ['pending', 'awaiting_approval', 'queued', 'running'])
            ->exists();

        if ($activeTask) {
            throw new ApplicationConflictException('This application has active work and cannot be archived yet.');
        }

        $application->forceFill(['archived_at' => now()])->save();

        $this->audit->record($actor, AuditAction::ApplicationArchived, AuditOutcome::Success, [
            'resource_kind' => 'application',
            'resource_id' => $application->getKey(),
            'metadata' => ['operation' => 'application.archive'],
        ]);

        return $application;
    }

    /**
     * @param  array<string, mixed>  $settings
     * @return array<string, mixed>
     */
    private function validateSettings(array $settings): array
    {
        $allowed = ['runtime', 'runtime_version', 'build_command', 'start_command', 'workers', 'scheduler', 'health_path', 'persistent_paths'];
        $extra = array_diff(array_keys($settings), $allowed);

        if ($extra !== []) {
            throw ValidationException::withMessages(['settings' => 'Unknown configuration keys: '.implode(', ', $extra).'.']);
        }

        $maxPaths = (int) config('hostnur.applications.max_persistent_paths', 20);

        return Validator::make($settings, [
            'runtime' => ['required', 'string', Rule::in(config('hostnur.applications.runtimes'))],
            'runtime_version' => ['nullable', 'string', 'max:32'],
            'build_command' => ['nullable', 'string', 'max:500'],
            'start_command' => ['nullable', 'string', 'max:500'],
            'workers' => ['nullable', 'integer', 'min:0', 'max:10'],
            'scheduler' => ['sometimes', 'boolean'],
            'health_path' => ['nullable', 'string', 'max:255'],
            'persistent_paths' => ['sometimes', 'array', 'max:'.$maxPaths],
            'persistent_paths.*' => ['string', 'max:255'],
        ])->validate();
    }

    /**
     * @param  array<string, mixed>  $settings
     * @return array<string, mixed>
     */
    private function normalizedSettings(array $settings): array
    {
        $health = $settings['health_path'] ?? null;
        $paths = $settings['persistent_paths'] ?? [];

        if (is_string($health) && $health !== '') {
            $this->assertRelativePath($health, 'health_path');
        } else {
            $health = null;
        }

        $paths = array_values(array_map(function (mixed $path): string {
            $this->assertRelativePath((string) $path, 'persistent_paths');

            return trim((string) $path);
        }, is_array($paths) ? $paths : []));

        return [
            'runtime' => $settings['runtime'],
            'runtime_version' => ($settings['runtime_version'] ?? null) ?: null,
            'build_command' => ($settings['build_command'] ?? null) ?: null,
            'start_command' => ($settings['start_command'] ?? null) ?: null,
            'workers' => (int) ($settings['workers'] ?? 0),
            'scheduler' => (bool) ($settings['scheduler'] ?? false),
            'health_path' => $health,
            'persistent_paths' => array_values(array_unique($paths)),
        ];
    }

    private function assertRelativePath(string $path, string $field): void
    {
        $path = trim($path);

        if ($path === ''
            || str_starts_with($path, '/')
            || str_starts_with($path, '~')
            || str_contains($path, '..')
            || preg_match('/[\x00-\x1F\x7F]/', $path) === 1) {
            throw ValidationException::withMessages([$field => 'Paths must be relative to the application, without leading slashes or "..".']);
        }
    }

    /**
     * @return array<string, mixed>
     */
    private function credentialVersions(Application $application): array
    {
        return [
            'source_credential' => $application->sourceCredential?->version,
            'secrets' => $application->secrets()
                ->pluck('version', 'name')
                ->all(),
        ];
    }
}
