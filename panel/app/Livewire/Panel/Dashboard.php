<?php

namespace App\Livewire\Panel;

use App\Agent\Dto\ServerSnapshot;
use App\Contracts\AgentClient;
use App\Enums\TaskStatus;
use App\Models\Task;
use Illuminate\Contracts\View\View;
use Illuminate\Support\Facades\Cache;
use Livewire\Attributes\Layout;
use Livewire\Component;

#[Layout('layouts.panel')]
class Dashboard extends Component
{
    public function render(): View
    {
        $snapshot = $this->snapshot();

        return view('livewire.panel.dashboard', [
            'snapshot' => $snapshot,
            'metrics' => collect($snapshot->metrics)->keyBy('name'),
            'staleAfter' => (int) config('hostnur.dashboard.stale_after_seconds', 120),
            'activeTasks' => Task::query()->whereIn('status', [
                TaskStatus::Pending->value,
                TaskStatus::AwaitingApproval->value,
                TaskStatus::Queued->value,
                TaskStatus::Running->value,
            ])->count(),
            'recentFailures' => Task::query()
                ->where('status', TaskStatus::Failed->value)
                ->latest('finished_at')
                ->limit(5)
                ->get(),
        ]);
    }

    private function snapshot(): ServerSnapshot
    {
        $seconds = (int) config('hostnur.dashboard.cache_seconds', 15);

        return Cache::remember(
            'hostnur.dashboard.snapshot.'.config('hostnur.agent_mode'),
            now()->addSeconds($seconds),
            fn (): ServerSnapshot => app(AgentClient::class)->snapshot(),
        );
    }
}
