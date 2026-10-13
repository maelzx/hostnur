<?php

namespace App\Livewire\Panel;

use App\Models\Deployment;
use Illuminate\Contracts\View\View;
use Livewire\Attributes\Layout;
use Livewire\Component;
use Livewire\WithPagination;

#[Layout('layouts.panel')]
class DeploymentIndex extends Component
{
    use WithPagination;

    public function render(): View
    {
        return view('livewire.panel.deployment-index', [
            'deployments' => Deployment::query()->with('application')->latest('created_at')->paginate(25),
        ]);
    }
}
