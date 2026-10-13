<?php

namespace App\Livewire\Panel;

use App\Models\Task;
use Illuminate\Contracts\View\View;
use Livewire\Attributes\Layout;
use Livewire\Component;

#[Layout('layouts.app')]
class TaskShow extends Component
{
    public string $taskId = '';

    public function mount(string $task): void
    {
        $this->taskId = $task;
    }

    public function render(): View
    {
        $task = Task::query()->with('events')->findOrFail($this->taskId);

        return view('livewire.panel.task-show', ['task' => $task]);
    }
}
