<?php

namespace App\Livewire\Panel;

use App\Enums\DomainHttpsPolicy;
use App\Exceptions\ApplicationConflictException;
use App\Models\Application;
use App\Models\SourceCredential;
use App\Models\User;
use App\Services\ApplicationService;
use Illuminate\Contracts\View\View;
use Livewire\Attributes\Layout;
use Livewire\Component;

#[Layout('layouts.panel')]
class ApplicationShow extends Component
{
    public string $applicationId = '';

    // Configuration
    public string $runtime = '';

    public string $runtime_version = '';

    public string $build_command = '';

    public string $start_command = '';

    public int $workers = 0;

    public bool $scheduler = false;

    public string $health_path = '';

    public string $persistent_paths_text = '';

    // Source
    public string $repository_url = '';

    public string $git_ref = '';

    public string $source_credential_id = '';

    // New credential
    public string $credential_label = '';

    public string $credential_type = 'https_token';

    public string $credential_secret = '';

    /** @var array<string, string> */
    public array $rotate_secrets = [];

    // Secret
    public string $secret_name = '';

    public string $secret_value = '';

    // Domain
    public string $hostname = '';

    public string $https_policy = 'auto';

    public function mount(string $application): void
    {
        $this->applicationId = $application;
        $application = $this->application();

        $this->authorize('view', $application);

        $settings = $application->revisions()->first()?->settings ?? [];

        $this->runtime = $application->runtime;
        $this->runtime_version = (string) $application->runtime_version;
        $this->build_command = (string) ($settings['build_command'] ?? '');
        $this->start_command = (string) ($settings['start_command'] ?? '');
        $this->workers = (int) ($settings['workers'] ?? 0);
        $this->scheduler = (bool) ($settings['scheduler'] ?? false);
        $this->health_path = (string) ($settings['health_path'] ?? '');
        $this->persistent_paths_text = implode("\n", $settings['persistent_paths'] ?? []);
        $this->repository_url = (string) $application->repository_url;
        $this->git_ref = (string) $application->git_ref;
        $this->source_credential_id = (string) $application->source_credential_id;
    }

    public function saveConfiguration(ApplicationService $service): void
    {
        $application = $this->application();
        $this->authorize('update', $application);

        $service->updateConfiguration($application->refresh(), [
            'runtime' => $this->runtime,
            'runtime_version' => $this->runtime_version !== '' ? $this->runtime_version : null,
            'build_command' => $this->build_command !== '' ? $this->build_command : null,
            'start_command' => $this->start_command !== '' ? $this->start_command : null,
            'workers' => $this->workers,
            'scheduler' => $this->scheduler,
            'health_path' => $this->health_path !== '' ? $this->health_path : null,
            'persistent_paths' => array_values(array_filter(array_map('trim', preg_split('/\r\n|\r|\n/', $this->persistent_paths_text) ?: []))),
        ]);

        session()->flash('status', 'Configuration saved as a new revision.');
        $this->redirect(route('panel.applications.show', ['application' => $this->applicationId]));
    }

    public function saveSource(ApplicationService $service): void
    {
        $application = $this->application();
        $this->authorize('manageSource', $application);

        $service->updateSource($application, [
            'repository_url' => $this->repository_url,
            'git_ref' => $this->git_ref !== '' ? $this->git_ref : null,
            'source_credential_id' => $this->source_credential_id !== '' ? $this->source_credential_id : null,
        ]);

        session()->flash('status', 'Source saved. No connectivity check was performed.');
        $this->redirect(route('panel.applications.show', ['application' => $this->applicationId]));
    }

    public function storeCredential(ApplicationService $service): void
    {
        $this->authorize('manageSource', $this->application());

        $service->storeCredential($this->actor(), [
            'label' => $this->credential_label,
            'type' => $this->credential_type,
            'secret' => $this->credential_secret,
        ]);

        $this->reset('credential_label', 'credential_secret');
        $this->credential_type = 'https_token';
        session()->flash('status', 'Credential stored encrypted.');
        $this->redirect(route('panel.applications.show', ['application' => $this->applicationId]));
    }

    public function rotateCredential(ApplicationService $service, string $credentialId): void
    {
        $this->authorize('manageSource', $this->application());

        $credential = SourceCredential::query()->findOrFail($credentialId);
        $service->rotateCredential($this->actor(), $credential, (string) ($this->rotate_secrets[$credentialId] ?? ''));

        unset($this->rotate_secrets[$credentialId]);
        session()->flash('status', 'Credential rotated.');
        $this->redirect(route('panel.applications.show', ['application' => $this->applicationId]));
    }

    public function setSecret(ApplicationService $service): void
    {
        $application = $this->application();
        $this->authorize('manageSecrets', $application);

        $service->setSecret($this->actor(), $application, $this->secret_name, $this->secret_value);

        $this->reset('secret_name', 'secret_value');
        session()->flash('status', 'Secret stored. Its value will not be shown again.');
        $this->redirect(route('panel.applications.show', ['application' => $this->applicationId]));
    }

    public function deleteSecret(ApplicationService $service, string $name): void
    {
        $application = $this->application();
        $this->authorize('manageSecrets', $application);

        $service->deleteSecret($this->actor(), $application, $name);

        session()->flash('status', 'Secret deleted.');
        $this->redirect(route('panel.applications.show', ['application' => $this->applicationId]));
    }

    public function addDomain(ApplicationService $service): void
    {
        $application = $this->application();
        $this->authorize('manageDomains', $application);

        $service->addDomain($this->actor(), $application, $this->hostname, DomainHttpsPolicy::from($this->https_policy));

        $this->reset('hostname');
        $this->https_policy = 'auto';
        session()->flash('status', 'Domain added.');
        $this->redirect(route('panel.applications.show', ['application' => $this->applicationId]));
    }

    public function removeDomain(ApplicationService $service, string $domainId): void
    {
        $application = $this->application();
        $this->authorize('manageDomains', $application);

        $domain = $application->domains()->findOrFail($domainId);
        $service->removeDomain($this->actor(), $domain);

        session()->flash('status', 'Domain removed.');
        $this->redirect(route('panel.applications.show', ['application' => $this->applicationId]));
    }

    public function archive(ApplicationService $service): void
    {
        $application = $this->application();
        $this->authorize('archive', $application);

        try {
            $service->archive($this->actor(), $application);
        } catch (ApplicationConflictException $exception) {
            $this->addError('archive', $exception->getMessage());

            return;
        }

        session()->flash('status', 'Application archived. History is retained.');
        $this->redirect(route('panel.applications.show', ['application' => $this->applicationId]));
    }

    public function render(): View
    {
        $application = $this->application();

        return view('livewire.panel.application-show', [
            'application' => $application,
            'revisions' => $application->revisions()->limit(10)->get(),
            'secrets' => $application->secrets()->get(),
            'domains' => $application->domains()->get(),
            'credentials' => SourceCredential::query()->latest('created_at')->get(),
            'runtimes' => config('hostnur.applications.runtimes'),
            'policies' => DomainHttpsPolicy::cases(),
        ]);
    }

    private function application(): Application
    {
        return Application::query()->with('sourceCredential')->findOrFail($this->applicationId);
    }

    private function actor(): User
    {
        return auth()->user();
    }
}
