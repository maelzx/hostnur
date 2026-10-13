<?php

namespace App\Livewire\Panel;

use App\Models\Application;
use App\Models\Backup;
use App\Models\BackupPolicy;
use App\Models\User;
use App\Services\BackupService;
use Illuminate\Contracts\View\View;
use Illuminate\Support\Str;
use Livewire\Attributes\Layout;
use Livewire\Component;

#[Layout('layouts.panel')]
class BackupIndex extends Component
{
    public string $scope = 'application';

    public string $applicationId = '';

    public string $expression = '0 2 * * *';

    public string $timezone = 'UTC';

    public int $retention_days = 7;

    public string $destination_credential_id = '';

    public bool $enabled = true;

    public string $backupKey = '';

    public function mount(): void
    {
        $this->backupKey = (string) Str::uuid();
    }

    public function createPolicy(BackupService $service): void
    {
        $this->authorize('viewAny', Application::class);

        $service->savePolicy($this->actor(), [
            'scope' => $this->scope,
            'application_id' => $this->applicationId !== '' ? $this->applicationId : null,
            'expression' => $this->expression,
            'timezone' => $this->timezone,
            'retention_days' => $this->retention_days,
            'destination_credential_id' => $this->destination_credential_id !== '' ? $this->destination_credential_id : null,
            'enabled' => $this->enabled,
        ]);

        session()->flash('status', 'Backup policy saved. Actual backups run through the agent later.');
        $this->redirect(route('panel.backups.index'));
    }

    public function requestBackup(BackupService $service, string $policyId): void
    {
        $this->authorize('viewAny', Application::class);
        $policy = BackupPolicy::query()->findOrFail($policyId);
        $service->requestBackup($this->actor(), $policy, $this->backupKey);

        session()->flash('status', 'Backup requested (simulated while the agent is unavailable).');
        $this->redirect(route('panel.backups.index'));
    }

    public function deletePolicy(BackupService $service, string $policyId): void
    {
        $this->authorize('viewAny', Application::class);
        $service->deletePolicy($this->actor(), BackupPolicy::query()->findOrFail($policyId));

        session()->flash('status', 'Backup policy deleted.');
        $this->redirect(route('panel.backups.index'));
    }

    public function render(BackupService $service): View
    {
        $backups = Backup::query()->with('application', 'policy')->latest('created_at')->get()
            ->map(fn (Backup $backup): Backup => $service->syncState($backup));

        return view('livewire.panel.backup-index', [
            'applications' => Application::query()->whereNull('archived_at')->orderBy('name')->get(),
            'policies' => BackupPolicy::query()->with('application', 'destinationCredential')->latest('created_at')->get(),
            'backups' => $backups,
            'credentials' => $service->destinationCredentials(),
        ]);
    }

    private function actor(): User
    {
        return auth()->user();
    }
}
