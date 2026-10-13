<?php

namespace App\Livewire\Panel;

use App\Enums\ServiceAction;
use App\Models\User;
use App\Services\ServerService;
use Illuminate\Contracts\View\View;
use Illuminate\Support\Str;
use Livewire\Attributes\Layout;
use Livewire\Component;

#[Layout('layouts.panel')]
class ServerOverview extends Component
{
    public string $serviceId = '';

    public string $service_action = 'restart';

    public string $packageId = '';

    public string $controlKey = '';

    public string $installKey = '';

    public function mount(): void
    {
        $this->controlKey = (string) Str::uuid();
        $this->installKey = (string) Str::uuid();
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

    public function render(ServerService $service): View
    {
        $snapshot = $service->snapshot();

        return view('livewire.panel.server-overview', [
            'snapshot' => $snapshot,
            'packages' => config('hostnur.server.packages'),
        ]);
    }

    private function actor(): User
    {
        return auth()->user();
    }
}
