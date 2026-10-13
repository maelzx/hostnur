<?php

namespace App\Livewire\Panel;

use App\Models\AuditEvent;
use Illuminate\Contracts\View\View;
use Livewire\Attributes\Layout;
use Livewire\Component;

#[Layout('layouts.panel')]
class AuditShow extends Component
{
    public string $eventId = '';

    public function mount(string $event): void
    {
        $this->eventId = $event;
    }

    public function render(): View
    {
        return view('livewire.panel.audit-show', [
            'event' => AuditEvent::query()->with('actor')->findOrFail($this->eventId),
        ]);
    }
}
