<?php

namespace App\Livewire\Panel;

use App\Enums\TaskStatus;
use App\Models\Task;
use Illuminate\Contracts\View\View;
use Livewire\Attributes\Layout;
use Livewire\Component;
use Livewire\WithPagination;

#[Layout('layouts.panel')]
class TaskIndex extends Component
{
    use WithPagination;

    public string $status = '';

    public string $operation = '';

    public function updatedStatus(): void
    {
        $this->resetPage();
    }

    public function updatedOperation(): void
    {
        $this->resetPage();
    }

    public function render(): View
    {
        $tasks = Task::query()
            ->when($this->status !== '', fn ($query) => $query->where('status', $this->status))
            ->when($this->operation !== '', fn ($query) => $query->where('operation', 'like', '%'.$this->operation.'%'))
            ->latest('created_at')
            ->paginate(25);

        return view('livewire.panel.task-index', [
            'tasks' => $tasks,
            'statuses' => TaskStatus::cases(),
        ]);
    }
}
