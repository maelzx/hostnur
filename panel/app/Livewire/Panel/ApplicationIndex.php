<?php

namespace App\Livewire\Panel;

use App\Models\Application;
use App\Models\User;
use App\Services\ApplicationService;
use Illuminate\Contracts\View\View;
use Livewire\Attributes\Layout;
use Livewire\Component;
use Livewire\WithPagination;

#[Layout('layouts.panel')]
class ApplicationIndex extends Component
{
    use WithPagination;

    public string $name = '';

    public string $slug = '';

    public string $provider = 'laravel';

    public string $runtime = 'php';

    public string $runtime_version = '';

    public function create(ApplicationService $service): void
    {
        $this->authorize('create', Application::class);

        $service->create($this->actor(), [
            'name' => $this->name,
            'slug' => $this->slug,
            'provider' => $this->provider,
            'runtime' => $this->runtime,
            'runtime_version' => $this->runtime_version !== '' ? $this->runtime_version : null,
        ]);

        $this->reset('name', 'slug', 'runtime_version');
        $this->provider = 'laravel';
        $this->runtime = 'php';

        session()->flash('status', 'Application created.');
        $this->redirect(route('panel.applications.index'));
    }

    public function render(): View
    {
        return view('livewire.panel.application-index', [
            'applications' => Application::query()->withCount('domains')->latest('created_at')->paginate(25),
            'providers' => config('hostnur.applications.providers'),
            'runtimes' => config('hostnur.applications.runtimes'),
        ]);
    }

    private function actor(): User
    {
        return auth()->user();
    }
}
