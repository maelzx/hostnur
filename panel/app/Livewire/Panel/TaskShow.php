<?php

namespace App\Livewire\Panel;

use App\Models\Task;
use Illuminate\Contracts\View\View;
use Livewire\Attributes\Layout;
use Livewire\Component;

#[Layout('layouts.panel')]
class TaskShow extends Component
{
    public string $taskId = '';

    public int $eventLimit = 50;

    public function mount(string $task): void
    {
        $this->taskId = $task;
    }

    public function loadMoreEvents(): void
    {
        $this->eventLimit += 50;
    }

    public function render(): View
    {
        $task = Task::query()->findOrFail($this->taskId);

        $events = $task->events()->limit($this->eventLimit)->get();
        $totalEvents = $task->events()->count();

        return view('livewire.panel.task-show', [
            'task' => $task,
            'events' => $events,
            'totalEvents' => $totalEvents,
            'hasMoreEvents' => $totalEvents > $events->count(),
            'pollWhileActive' => $task->status->isActive(),
        ]);
    }
}
