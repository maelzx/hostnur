<?php

namespace App\Livewire\Panel;

use App\Models\Task;
use Illuminate\Contracts\View\View;
use Livewire\Attributes\Layout;
use Livewire\Component;
use Livewire\WithPagination;

#[Layout('layouts.app')]
class TaskIndex extends Component
{
    use WithPagination;

    public function render(): View
    {
        return view('livewire.panel.task-index', [
            'tasks' => Task::query()->latest('created_at')->paginate(25),
        ]);
    }
}
