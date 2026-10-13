<?php

namespace App\Livewire\Panel;

use App\Models\Application;
use App\Models\DatabaseRequest;
use App\Models\User;
use App\Services\DatabaseService;
use Illuminate\Contracts\View\View;
use Illuminate\Support\Str;
use Livewire\Attributes\Layout;
use Livewire\Component;

#[Layout('layouts.panel')]
class DatabaseIndex extends Component
{
    public string $applicationId = '';

    public string $engine = 'mariadb';

    public string $database_name = '';

    public string $username = '';

    public string $requestKey = '';

    public function mount(): void
    {
        $this->requestKey = (string) Str::uuid();
    }

    public function request(DatabaseService $service): void
    {
        $application = Application::query()->findOrFail($this->applicationId);
        $this->authorize('update', $application);

        $service->request($this->actor(), $application, $this->engine, $this->database_name, $this->username, $this->requestKey);

        $this->reset('database_name', 'username');
        session()->flash('status', 'Database provisioning requested with generated credentials.');
        $this->redirect(route('panel.databases.index'));
    }

    public function render(DatabaseService $service): View
    {
        $requests = DatabaseRequest::query()->with('application')->latest('created_at')->get()
            ->map(fn (DatabaseRequest $request): DatabaseRequest => $service->syncState($request));

        return view('livewire.panel.database-index', [
            'applications' => Application::query()->whereNull('archived_at')->orderBy('name')->get(),
            'requests' => $requests,
            'engines' => config('hostnur.server.database_engines'),
        ]);
    }

    private function actor(): User
    {
        return auth()->user();
    }
}
