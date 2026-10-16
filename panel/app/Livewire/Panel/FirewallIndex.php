<?php

namespace App\Livewire\Panel;

use App\Services\ServerService;
use Illuminate\Contracts\View\View;
use Livewire\Attributes\Layout;
use Livewire\Component;

#[Layout('layouts.panel')]
class FirewallIndex extends Component
{
    public function render(ServerService $service): View
    {
        return view('livewire.panel.firewall-index', [
            'snapshot' => $service->snapshot(),
            'staleAfter' => (int) config('hostnur.dashboard.stale_after_seconds', 120),
        ]);
    }
}
