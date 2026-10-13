<?php

namespace App\Livewire\Panel;

use App\Enums\DeploymentState;
use App\Models\Deployment;
use App\Services\DeploymentService;
use App\Services\TaskService;
use Illuminate\Contracts\View\View;
use Livewire\Attributes\Layout;
use Livewire\Component;

#[Layout('layouts.panel')]
class DeploymentShow extends Component
{
    public string $deploymentId = '';

    public int $eventLimit = 50;

    public function mount(string $deployment): void
    {
        $this->deploymentId = $deployment;
    }

    public function loadMoreEvents(): void
    {
        $this->eventLimit += 50;
    }

    public function cancel(TaskService $tasks): void
    {
        $deployment = $this->deployment();

        if ($deployment->task === null) {
            return;
        }

        $tasks->cancel(auth()->user(), $deployment->task->refresh());

        session()->flash('status', 'Cancellation requested.');
        $this->redirect(route('panel.deployments.show', ['deployment' => $this->deploymentId]));
    }

    public function render(DeploymentService $service): View
    {
        $deployment = $this->deployment();
        $deployment = $service->refresh($deployment);

        $task = $deployment->task;
        $events = $task !== null ? $task->events()->limit($this->eventLimit)->get() : collect();
        $totalEvents = $task !== null ? $task->events()->count() : 0;

        return view('livewire.panel.deployment-show', [
            'deployment' => $deployment,
            'task' => $task,
            'events' => $events,
            'totalEvents' => $totalEvents,
            'hasMoreEvents' => $totalEvents > $events->count(),
            'pollWhileActive' => $deployment->state === DeploymentState::Running
                || $deployment->state === DeploymentState::Requested,
        ]);
    }

    private function deployment(): Deployment
    {
        return Deployment::query()->with(['application', 'task'])->findOrFail($this->deploymentId);
    }
}
