<?php

namespace App\Livewire\Panel;

use App\Enums\DomainHttpsPolicy;
use App\Exceptions\DeploymentPreflightException;
use App\Models\Application;
use App\Models\SourceCredential;
use App\Models\User;
use App\Services\ApplicationService;
use App\Services\CertificateService;
use App\Services\DatabaseService;
use App\Services\DeploymentService;
use Illuminate\Contracts\View\View;
use Illuminate\Support\Str;
use Illuminate\Validation\Rule;
use Illuminate\Validation\ValidationException;
use Livewire\Attributes\Layout;
use Livewire\Component;

/**
 * Guided creation wizard: collects the intent for an application, its source,
 * runtime, domain (+ optional TLS) and an optional managed database, then
 * persists everything through the existing services in one pass so the operator
 * does not have to hop between screens. Every optional step can be left blank.
 */
#[Layout('layouts.panel')]
class ApplicationCreate extends Component
{
    /** @var list<string> */
    public const STEPS = ['Basics', 'Source', 'Runtime', 'Domain & TLS', 'Database', 'Review'];

    public int $step = 0;

    // Basics
    public string $name = '';

    public string $slug = '';

    public string $provider = 'laravel';

    public string $runtime = 'php';

    public string $runtime_version = '';

    // Source
    public string $repository_url = '';

    public string $git_ref = '';

    public string $source_credential_id = '';

    // Runtime, build and scale
    public string $build_command = '';

    public string $start_command = '';

    public int $workers = 0;

    public bool $scheduler = false;

    public string $health_path = '';

    public string $persistent_paths_text = '';

    // Domain and TLS
    public string $hostname = '';

    public string $https_policy = 'auto';

    public bool $request_certificate = true;

    // Database
    public bool $provision_database = false;

    public string $engine = 'mariadb';

    public string $database_name = '';

    public string $database_username = '';

    // Review
    public bool $run_deploy = false;

    public string $databaseKey = '';

    public string $certificateKey = '';

    public string $deployKey = '';

    public function mount(): void
    {
        $this->authorize('create', Application::class);

        $this->databaseKey = (string) Str::uuid();
        $this->certificateKey = (string) Str::uuid();
        $this->deployKey = (string) Str::uuid();
    }

    public function next(): void
    {
        $this->validateStep($this->step);
        $this->step = min($this->step + 1, count(self::STEPS) - 1);
    }

    public function back(): void
    {
        $this->resetErrorBag();
        $this->step = max($this->step - 1, 0);
    }

    public function goTo(int $step): void
    {
        if ($step >= 0 && $step < $this->step) {
            $this->resetErrorBag();
            $this->step = $step;
        }
    }

    public function applyLaravelDefaults(): void
    {
        $this->runtime = 'php';

        if (trim($this->runtime_version) === '') {
            $this->runtime_version = '8.3';
        }

        if (trim($this->health_path) === '') {
            $this->health_path = 'up';
        }

        if ($this->workers < 1) {
            $this->workers = 1;
        }

        $this->scheduler = true;

        if (trim($this->persistent_paths_text) === '') {
            $this->persistent_paths_text = 'storage/app';
        }
    }

    public function finish(
        ApplicationService $applications,
        CertificateService $certificates,
        DatabaseService $databases,
        DeploymentService $deployments,
    ): void {
        $this->authorize('create', Application::class);

        foreach (range(0, count(self::STEPS) - 1) as $step) {
            $this->validateStep($step);
        }

        $actor = $this->actor();

        try {
            $application = $applications->create($actor, [
                'name' => $this->name,
                'slug' => $this->slug,
                'provider' => $this->provider,
                'runtime' => $this->runtime,
                'runtime_version' => $this->runtime_version !== '' ? $this->runtime_version : null,
            ]);
        } catch (ValidationException $exception) {
            $this->mapValidationErrors($exception);

            return;
        }

        try {
            if (trim($this->repository_url) !== '') {
                $applications->updateSource($actor, $application, [
                    'repository_url' => $this->repository_url,
                    'git_ref' => $this->git_ref !== '' ? $this->git_ref : null,
                    'source_credential_id' => $this->source_credential_id !== '' ? $this->source_credential_id : null,
                ]);
            }

            if ($this->hasRuntimeOverrides()) {
                $applications->updateConfiguration($actor, $application->refresh(), [
                    'runtime' => $this->runtime,
                    'runtime_version' => $this->runtime_version !== '' ? $this->runtime_version : null,
                    'build_command' => $this->build_command !== '' ? $this->build_command : null,
                    'start_command' => $this->start_command !== '' ? $this->start_command : null,
                    'workers' => $this->workers,
                    'scheduler' => $this->scheduler,
                    'health_path' => $this->health_path !== '' ? $this->health_path : null,
                    'persistent_paths' => $this->persistentPaths(),
                ]);
            }

            if (trim($this->hostname) !== '') {
                $domain = $applications->addDomain(
                    $actor,
                    $application,
                    $this->hostname,
                    DomainHttpsPolicy::from($this->https_policy),
                );

                if ($this->request_certificate) {
                    $certificates->requestManaged($actor, $domain, $this->certificateKey);
                }
            }

            if ($this->provision_database) {
                $databases->request(
                    $actor,
                    $application,
                    $this->engine,
                    $this->database_name,
                    $this->database_username,
                    $this->databaseKey,
                );
            }
        } catch (ValidationException $exception) {
            $this->mapValidationErrors($exception);

            return;
        }

        $status = 'Application created.';

        if ($this->run_deploy && trim($this->repository_url) !== '') {
            try {
                $deployments->deploy($actor, $application->refresh(), $this->deployKey, true);
                $status = 'Application created and deployment requested.';
            } catch (DeploymentPreflightException $exception) {
                session()->flash('status', 'Application created, but the deployment was not requested: '.$exception->getMessage());
                $this->redirect(route('panel.applications.show', ['application' => $application->getKey()]));

                return;
            }
        }

        session()->flash('status', $status);
        $this->redirect(route('panel.applications.show', ['application' => $application->getKey()]));
    }

    public function render(): View
    {
        return view('livewire.panel.application-create', [
            'steps' => self::STEPS,
            'providers' => config('hostnur.applications.providers'),
            'runtimes' => config('hostnur.applications.runtimes'),
            'engines' => config('hostnur.server.database_engines'),
            'policies' => DomainHttpsPolicy::cases(),
            'credentials' => SourceCredential::query()->latest('created_at')->get(),
        ]);
    }

    private function validateStep(int $step): void
    {
        match ($step) {
            0 => $this->validate([
                'name' => ['required', 'string', 'max:255'],
                'slug' => ['required', 'string', 'min:3', 'max:63', 'regex:/^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/', Rule::unique('applications', 'slug')],
                'provider' => ['required', 'string', Rule::in(config('hostnur.applications.providers'))],
                'runtime' => ['required', 'string', Rule::in(config('hostnur.applications.runtimes'))],
                'runtime_version' => ['nullable', 'string', 'max:32'],
            ], [], ['name' => 'display name', 'slug' => 'slug', 'runtime_version' => 'runtime version']),
            1 => $this->validate([
                'repository_url' => ['nullable', 'string', 'max:500'],
                'git_ref' => ['nullable', 'string', 'max:255', 'regex:/^[A-Za-z0-9][A-Za-z0-9._\/-]*$/'],
                'source_credential_id' => ['nullable', 'uuid', Rule::exists('source_credentials', 'id')],
            ], [], ['repository_url' => 'repository URL', 'git_ref' => 'Git ref', 'source_credential_id' => 'credential']),
            2 => $this->validate([
                'build_command' => ['nullable', 'string', 'max:500'],
                'start_command' => ['nullable', 'string', 'max:500'],
                'workers' => ['required', 'integer', 'min:0', 'max:10'],
                'scheduler' => ['boolean'],
                'health_path' => ['nullable', 'string', 'max:255'],
                'persistent_paths_text' => ['nullable', 'string'],
            ]),
            3 => $this->validate([
                'hostname' => ['nullable', 'string', 'max:255'],
                'https_policy' => ['required', Rule::enum(DomainHttpsPolicy::class)],
                'request_certificate' => ['boolean'],
            ]),
            4 => $this->validate($this->provision_database ? [
                'engine' => ['required', 'string', Rule::in(config('hostnur.server.database_engines'))],
                'database_name' => ['required', 'string', 'regex:/^[a-z][a-z0-9_]{0,63}$/'],
                'database_username' => ['required', 'string', 'regex:/^[a-z][a-z0-9_]{0,63}$/'],
            ] : [
                'engine' => ['required', 'string', Rule::in(config('hostnur.server.database_engines'))],
            ]),
            default => null,
        };
    }

    /**
     * @return list<string>
     */
    private function persistentPaths(): array
    {
        return array_values(array_filter(array_map('trim', preg_split('/\r\n|\r|\n/', $this->persistent_paths_text) ?: [])));
    }

    private function hasRuntimeOverrides(): bool
    {
        return trim($this->build_command) !== ''
            || trim($this->start_command) !== ''
            || $this->workers > 0
            || $this->scheduler
            || trim($this->health_path) !== ''
            || trim($this->persistent_paths_text) !== '';
    }

    private function mapValidationErrors(ValidationException $exception): void
    {
        $map = [
            'name' => 0, 'slug' => 0, 'provider' => 0, 'runtime' => 0, 'runtime_version' => 0,
            'repository_url' => 1, 'git_ref' => 1, 'source_credential_id' => 1,
            'build_command' => 2, 'start_command' => 2, 'workers' => 2, 'scheduler' => 2, 'health_path' => 2, 'persistent_paths' => 2,
            'hostname' => 3, 'https_policy' => 3,
            'engine' => 4, 'database_name' => 4, 'username' => 4,
        ];

        $lowest = count(self::STEPS) - 1;

        foreach ($exception->errors() as $field => $messages) {
            $property = $field === 'username' ? 'database_username' : $field;
            $this->addError($property, $messages[0]);
            $step = $map[$field] ?? $lowest;
            $lowest = min($lowest, $step);
        }

        $this->step = $lowest;
    }

    private function actor(): User
    {
        return auth()->user();
    }
}
