<?php

namespace App\Livewire\Panel;

use App\Enums\AuditAction;
use App\Models\AuditEvent;
use Illuminate\Contracts\View\View;
use Livewire\Attributes\Layout;
use Livewire\Component;
use Livewire\WithPagination;

#[Layout('layouts.panel')]
class AuditIndex extends Component
{
    use WithPagination;

    public string $action = '';

    public string $actor = '';

    public function updatedAction(): void
    {
        $this->resetPage();
    }

    public function updatedActor(): void
    {
        $this->resetPage();
    }

    public function render(): View
    {
        $events = AuditEvent::query()
            ->with('actor')
            ->when($this->action !== '', fn ($query) => $query->where('action', $this->action))
            ->when($this->actor !== '', fn ($query) => $query->where('actor_label', 'like', '%'.$this->actor.'%'))
            ->latest('recorded_at')
            ->paginate(30);

        return view('livewire.panel.audit-index', [
            'events' => $events,
            'actions' => AuditAction::cases(),
        ]);
    }
}
