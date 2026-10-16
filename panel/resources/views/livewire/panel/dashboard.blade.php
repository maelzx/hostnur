@php
    use App\Enums\ApplicationStatus;

    $cpu = $metrics->get('cpu_percent');
    $disk = $metrics->get('disk_used_percent');
    $load = $metrics->get('load_1m');
    $uptime = $metrics->get('uptime_seconds');
    $memUsed = $metrics->get('memory_used_bytes');
    $memTotal = $metrics->get('memory_total_bytes');
    $memPercent = ($memUsed && $memTotal && $memTotal['value'] > 0)
        ? round($memUsed['value'] / $memTotal['value'] * 100, 1)
        : null;
    $dash = '—';
    $sampleHint = $snapshot->available ? 'Sampled '.$snapshot->sampledAt : 'No observation';
    $uptimeText = $uptime ? floor($uptime['value'] / 86400).'d '.floor(($uptime['value'] % 86400) / 3600).'h' : $dash;
    $deployedCount = $applicationCounts[ApplicationStatus::Deployed->value] ?? 0;
    $toneBg = ['success' => 'bg-emerald-500', 'info' => 'bg-indigo-500', 'danger' => 'bg-red-500', 'warning' => 'bg-amber-500', 'neutral' => 'bg-slate-400'];
@endphp

<main class="pb-4">
    <header class="mb-6">
        <h1 class="text-2xl font-semibold tracking-tight">Overview</h1>
        <p class="mt-2 text-sm leading-6 text-slate-600">Signed in as {{ auth()->user()?->name }}.</p>
    </header>

    @if (! $snapshot->available)
        <p role="status" class="mb-6 rounded-xl border border-slate-300 bg-slate-100 px-4 py-3 text-sm text-slate-700">
            The agent is unavailable. {{ $snapshot->reason }}
        </p>
    @elseif ($snapshot->isStale($staleAfter))
        <p role="status" class="mb-6 rounded-xl border border-amber-300 bg-amber-50 px-4 py-3 text-sm text-amber-900">
            The last observation is stale ({{ $snapshot->sampledAt }}).
        </p>
    @endif

    {{-- Server resources --}}
    <section aria-label="Server resources" class="grid grid-cols-1 gap-4 sm:grid-cols-3">
        <x-ui.gauge label="CPU" :value="$cpu['value'] ?? null" :hint="$sampleHint" />
        <x-ui.gauge label="Memory" :value="$memPercent" :hint="$sampleHint" />
        <x-ui.gauge label="Disk" :value="$disk['value'] ?? null" :hint="$sampleHint" />
    </section>

    <section aria-label="Server summary" class="mt-4 grid grid-cols-2 gap-4 lg:grid-cols-3">
        <x-ui.stat label="Load (1m)" :value="$load ? $load['value'] : $dash" :hint="$sampleHint" />
        <x-ui.stat label="Uptime" :value="$uptimeText" :hint="$sampleHint" />
        <x-ui.stat label="Active tasks" :value="$activeTasks" hint="Pending, queued or running" />
    </section>

    {{-- Applications --}}
    <section class="mt-6">
        <x-ui.panel title="Applications">
            @if ($applicationsTotal === 0)
                <p class="mt-2 text-sm text-slate-600">No applications yet.</p>
                <p class="mt-3 text-sm"><a href="{{ route('panel.applications.create') }}" class="font-medium text-indigo-700 hover:underline">Create your first application</a></p>
            @else
                <p class="mt-2 text-sm text-slate-600">
                    <span class="font-semibold text-slate-900">{{ $applicationsTotal }}</span> total ·
                    <span class="font-semibold text-slate-900">{{ $deployedCount }}</span> deployed
                </p>

                <div class="mt-4 flex h-2 overflow-hidden rounded-full bg-slate-100">
                    @foreach ($applicationStatuses as $status)
                        @php $count = $applicationCounts[$status->value] ?? 0; @endphp
                        @if ($count > 0)
                            <div class="{{ $toneBg[$status->tone()] ?? 'bg-slate-400' }}" style="width: {{ $count / $applicationsTotal * 100 }}%"></div>
                        @endif
                    @endforeach
                </div>

                <ul class="mt-4 grid grid-cols-2 gap-2 text-sm sm:grid-cols-3">
                    @foreach ($applicationStatuses as $status)
                        @php $count = $applicationCounts[$status->value] ?? 0; @endphp
                        @if ($count > 0)
                            <li class="flex items-center gap-2">
                                <span class="h-2.5 w-2.5 rounded-full {{ $toneBg[$status->tone()] ?? 'bg-slate-400' }}"></span>
                                <span class="text-slate-600">{{ $status->label() }}</span>
                                <span class="ml-auto font-semibold text-slate-900">{{ $count }}</span>
                            </li>
                        @endif
                    @endforeach
                </ul>

                <p class="mt-4 text-sm">
                    <a href="{{ route('panel.applications.index') }}" class="font-medium text-indigo-700 hover:underline">View all applications</a>
                </p>
            @endif
        </x-ui.panel>
    </section>

    <section aria-label="Host and services" class="mt-6 grid gap-4 lg:grid-cols-2">
        <x-ui.panel title="Host">
            <dl class="mt-4 space-y-2 text-sm">
                <div class="flex justify-between gap-4">
                    <dt class="text-slate-600">Hostname</dt>
                    <dd class="font-medium">{{ $snapshot->hostname ?? '—' }}</dd>
                </div>
                <div class="flex justify-between gap-4">
                    <dt class="text-slate-600">Operating system</dt>
                    <dd class="font-medium">{{ $snapshot->os ?? '—' }}</dd>
                </div>
            </dl>
        </x-ui.panel>

        <x-ui.panel title="Managed services">
            @if ($snapshot->services === [])
                <p class="mt-4 text-sm text-slate-600">No services reported.</p>
            @else
                <ul class="mt-4 space-y-2 text-sm">
                    @foreach ($snapshot->services as $service)
                        <li class="flex items-center justify-between gap-4">
                            <span class="font-medium">{{ $service['name'] }}</span>
                            <x-ui.badge :tone="$service['status'] === 'running' ? 'success' : 'warning'">{{ $service['status'] }}</x-ui.badge>
                        </li>
                    @endforeach
                </ul>
            @endif
        </x-ui.panel>
    </section>

    <section class="mt-6 grid gap-4 lg:grid-cols-2">
        <x-ui.panel title="Server updates">
            @if ($updates === null)
                <p class="mt-2 text-sm text-slate-600">Package update status is unavailable.</p>
            @elseif ($updates->updates === [])
                <p class="mt-2 text-sm text-slate-600">No pending updates.</p>
            @else
                <p class="mt-2 text-sm text-slate-600">
                    <span class="font-semibold text-slate-900">{{ count($updates->updates) }}</span>
                    pending update{{ count($updates->updates) === 1 ? '' : 's' }}.
                </p>
                <ul class="mt-3 divide-y divide-slate-100 text-sm">
                    @foreach (array_slice($updates->updates, 0, 8) as $update)
                        <li class="flex items-center justify-between gap-3 py-1.5">
                            <span class="font-mono">{{ $update['package'] }}</span>
                            <span class="flex items-center gap-2 text-xs text-slate-500">
                                <span class="font-mono">{{ $update['current'] }} → {{ $update['candidate'] }}</span>
                                @if ($update['security'])
                                    <x-ui.badge tone="danger">security</x-ui.badge>
                                @endif
                            </span>
                        </li>
                    @endforeach
                </ul>
                @if (count($updates->updates) > 8)
                    <p class="mt-2 text-xs text-slate-500">Showing 8 of {{ count($updates->updates) }}.</p>
                @endif
            @endif
        </x-ui.panel>

        <x-ui.panel title="Recent failures">
            @if ($recentFailures->isEmpty())
                <p class="mt-4 text-sm text-slate-600">No recent failed tasks.</p>
            @else
                <ul class="mt-4 divide-y divide-slate-100 text-sm">
                    @foreach ($recentFailures as $task)
                        <li class="flex items-center justify-between gap-4 py-2">
                            <a href="{{ route('panel.tasks.show', ['task' => $task->id]) }}" class="font-medium text-indigo-700 hover:underline">{{ $task->operation }}</a>
                            <span class="text-slate-500">{{ $task->error_code ?? 'failed' }}</span>
                        </li>
                    @endforeach
                </ul>
            @endif
            <p class="mt-4 text-sm">
                <a href="{{ route('panel.tasks.index') }}" class="font-medium text-indigo-700 hover:underline">View all tasks</a>
            </p>
        </x-ui.panel>
    </section>
</main>
