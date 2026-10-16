<?php

namespace App\Livewire\Panel;

use App\Agent\Dto\OperationRequest;
use App\Agent\Dto\ServerSnapshot;
use App\Agent\Dto\ServerUpdatesSnapshot;
use App\Contracts\AgentClient;
use App\Enums\AgentMode;
use App\Enums\ApplicationStatus;
use App\Enums\TaskStatus;
use App\Models\Application;
use App\Models\Task;
use Illuminate\Contracts\View\View;
use Illuminate\Support\Facades\Cache;
use Illuminate\Support\Str;
use Livewire\Attributes\Layout;
use Livewire\Component;

#[Layout('layouts.panel')]
class Dashboard extends Component
{
    public function render(): View
    {
        $snapshot = $this->snapshot();

        $applications = Application::query()->with('latestDeployment')->get();
        $applicationCounts = collect(ApplicationStatus::cases())
            ->mapWithKeys(fn (ApplicationStatus $status): array => [$status->value => 0]);

        foreach ($applications as $application) {
            $key = $application->status()->value;
            $applicationCounts[$key] = ($applicationCounts[$key] ?? 0) + 1;
        }

        return view('livewire.panel.dashboard', [
            'snapshot' => $snapshot,
            'updates' => $this->updates(app(AgentClient::class)),
            'metrics' => collect($snapshot->metrics)->keyBy('name'),
            'applicationsTotal' => $applications->count(),
            'applicationCounts' => $applicationCounts,
            'applicationStatuses' => ApplicationStatus::cases(),
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

    private function updates(AgentClient $agent): ?ServerUpdatesSnapshot
    {
        try {
            return $agent->serverUpdates(new OperationRequest(
                protocolVersion: (int) config('hostnur.protocol_version', 1),
                requestId: (string) Str::uuid(),
                idempotencyKey: (string) Str::uuid(),
                operation: 'server.updates',
                resource: null,
                parameters: [],
                timeoutSeconds: null,
            ));
        } catch (\Throwable) {
            return null;
        }
    }

    private function snapshot(): ServerSnapshot
    {
        $seconds = (int) config('hostnur.dashboard.cache_seconds', 15);

        try {
            $sample = Cache::remember(
                'hostnur.dashboard.snapshot.'.config('hostnur.agent_mode'),
                now()->addSeconds($seconds),
                fn (): array => app(AgentClient::class)->snapshot()->toArray(),
            );

            return ServerSnapshot::fromArray($sample);
        } catch (\Throwable) {
            // A busy/unreachable agent must not blank the whole page.
            return new ServerSnapshot(
                protocolVersion: (int) config('hostnur.protocol_version', 1),
                mode: AgentMode::Unavailable,
                available: false,
                sampledAt: now()->toIso8601String(),
                metrics: [],
                services: [],
                reason: 'The agent is unavailable or busy; retry shortly.',
            );
        }
    }
}
