<?php

namespace App\Livewire\Panel;

use App\Models\Application;
use Illuminate\Contracts\View\View;
use Livewire\Attributes\Layout;
use Livewire\Component;
use Livewire\WithPagination;

#[Layout('layouts.panel')]
class ApplicationIndex extends Component
{
    use WithPagination;

    public function render(): View
    {
        $this->authorize('viewAny', Application::class);

        return view('livewire.panel.application-index', [
            'applications' => Application::query()->with('latestDeployment')->withCount('domains')->latest('created_at')->paginate(25),
        ]);
    }
}
