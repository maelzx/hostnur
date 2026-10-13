<?php

namespace App\Livewire\Panel;

use App\Models\AuditEvent;
use Illuminate\Contracts\View\View;
use Livewire\Attributes\Layout;
use Livewire\Component;
use Livewire\WithPagination;

#[Layout('layouts.app')]
class AuditIndex extends Component
{
    use WithPagination;

    public function render(): View
    {
        return view('livewire.panel.audit-index', [
            'events' => AuditEvent::query()->with('actor')->latest('recorded_at')->paginate(30),
        ]);
    }
}
