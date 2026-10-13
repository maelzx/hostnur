<?php

namespace App\Livewire\Panel;

use App\Actions\Deployments\DeploymentPreflight;
use App\Exceptions\DeploymentPreflightException;
use App\Models\Application;
use App\Models\User;
use App\Services\DeploymentService;
use Illuminate\Contracts\View\View;
use Illuminate\Support\Str;
use Livewire\Attributes\Layout;
use Livewire\Component;

#[Layout('layouts.panel')]
class ApplicationOperations extends Component
{
    public string $applicationId = '';

    public bool $run_migrations = false;

    public ?string $confirmedFingerprint = null;

    public string $idempotencyKey = '';

    public string $restartKey = '';

    public string $rollbackKey = '';

    public string $releaseId = '';

    public function mount(string $application): void
    {
        $this->applicationId = $application;
        $application = $this->application();

        $this->authorize('update', $application);

        $this->idempotencyKey = (string) Str::uuid();
        $this->restartKey = (string) Str::uuid();
        $this->rollbackKey = (string) Str::uuid();
    }

    public function updatedRunMigrations(): void
    {
        $this->confirmedFingerprint = null;
    }

    public function confirmPlan(): void
    {
        $this->authorize('update', $this->application());
        $this->confirmedFingerprint = $this->fingerprint();
    }

    public function deploy(DeploymentService $service): void
    {
        $application = $this->application();
        $this->authorize('update', $application);

        if ($this->confirmedFingerprint !== $this->fingerprint()) {
            $this->addError('confirm', 'The plan changed. Review and confirm the current plan again.');

            return;
        }

        try {
            $deployment = $service->deploy($this->actor(), $application, $this->idempotencyKey, $this->run_migrations);
        } catch (DeploymentPreflightException $exception) {
            $this->addError('confirm', $exception->getMessage());

            return;
        }

        session()->flash('status', 'Deployment requested.');
        $this->redirect(route('panel.deployments.show', ['deployment' => $deployment->getKey()]));
    }

    public function restart(DeploymentService $service): void
    {
        $application = $this->application();
        $this->authorize('update', $application);

        try {
            $deployment = $service->restart($this->actor(), $application, $this->restartKey);
        } catch (DeploymentPreflightException $exception) {
            $this->addError('restart', $exception->getMessage());

            return;
        }

        session()->flash('status', 'Restart requested.');
        $this->redirect(route('panel.deployments.show', ['deployment' => $deployment->getKey()]));
    }

    public function rollback(DeploymentService $service): void
    {
        $application = $this->application();
        $this->authorize('update', $application);

        if ($this->releaseId === '') {
            $this->addError('release', 'Select a release to roll back to.');

            return;
        }

        try {
            $deployment = $service->rollback($this->actor(), $application, $this->releaseId, $this->rollbackKey);
        } catch (DeploymentPreflightException $exception) {
            $this->addError('release', $exception->getMessage());

            return;
        }

        session()->flash('status', 'Rollback requested.');
        $this->redirect(route('panel.deployments.show', ['deployment' => $deployment->getKey()]));
    }

    public function render(DeploymentPreflight $preflight, DeploymentService $service): View
    {
        $application = $this->application();

        return view('livewire.panel.application-operations', [
            'application' => $application,
            'checks' => $preflight->deployChecks($application),
            'revision' => $application->revisions()->first(),
            'releases' => $service->selectableReleases($application),
            'planFingerprint' => $this->fingerprint(),
            'confirmed' => $this->confirmedFingerprint === $this->fingerprint(),
        ]);
    }

    private function fingerprint(): string
    {
        $application = $this->application();
        $revision = $application->revisions()->first();

        return hash('sha256', json_encode([
            'application' => $application->getKey(),
            'revision' => $revision?->getKey(),
            'ref' => $application->git_ref,
            'run_migrations' => $this->run_migrations,
        ], JSON_THROW_ON_ERROR));
    }

    private function application(): Application
    {
        return Application::query()->findOrFail($this->applicationId);
    }

    private function actor(): User
    {
        return auth()->user();
    }
}
