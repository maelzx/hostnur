<?php

namespace App\Livewire\Panel;

use App\Enums\ServiceAction;
use App\Models\ApprovedPackage;
use App\Models\User;
use App\Services\PackageCatalogService;
use App\Services\ServerService;
use Illuminate\Contracts\View\View;
use Illuminate\Support\Str;
use Livewire\Attributes\Layout;
use Livewire\Component;

#[Layout('layouts.panel')]
class PackagesIndex extends Component
{
    public string $serviceId = '';

    public string $service_action = 'restart';

    public string $packageId = '';

    public string $packageName = '';

    public string $controlKey = '';

    public string $installKey = '';

    public string $upgradeKey = '';

    public function mount(): void
    {
        $this->controlKey = (string) Str::uuid();
        $this->installKey = (string) Str::uuid();
        $this->upgradeKey = (string) Str::uuid();
    }

    public function control(ServerService $service): void
    {
        $task = $service->control($this->actor(), $this->serviceId, ServiceAction::from($this->service_action), $this->controlKey);

        session()->flash('status', 'Service control requested.');
        $this->redirect(route('panel.tasks.show', ['task' => $task->getKey()]));
    }

    public function install(ServerService $service): void
    {
        $task = $service->installPackage($this->actor(), $this->packageId, $this->installKey);

        session()->flash('status', 'Package install requested.');
        $this->redirect(route('panel.tasks.show', ['task' => $task->getKey()]));
    }

    public function addPackage(PackageCatalogService $catalog): void
    {
        $catalog->add($this->actor(), $this->packageName);

        $this->reset('packageName');
        session()->flash('status', 'Package approved.');
        $this->redirect(route('panel.packages'));
    }

    public function removePackage(PackageCatalogService $catalog, string $packageId): void
    {
        $package = ApprovedPackage::query()->findOrFail($packageId);
        $catalog->remove($this->actor(), $package);

        session()->flash('status', 'Package removed from the catalog.');
        $this->redirect(route('panel.packages'));
    }

    public function applySecurity(ServerService $service): void
    {
        $this->applyUpdates($service, true);
    }

    public function applyAll(ServerService $service): void
    {
        $this->applyUpdates($service, false);
    }

    private function applyUpdates(ServerService $service, bool $securityOnly): void
    {
        $task = $service->upgradePackages($this->actor(), $securityOnly, $this->upgradeKey);

        session()->flash('status', 'Package upgrade requested.');
        $this->redirect(route('panel.tasks.show', ['task' => $task->getKey()]));
    }

    public function render(ServerService $service): View
    {
        return view('livewire.panel.packages-index', [
            'snapshot' => $service->snapshot(),
            'packages' => config('hostnur.server.packages'),
            'catalog' => ApprovedPackage::query()->orderBy('name')->get(),
            'staleAfter' => (int) config('hostnur.dashboard.stale_after_seconds', 120),
        ]);
    }

    private function actor(): User
    {
        return auth()->user();
    }
}
