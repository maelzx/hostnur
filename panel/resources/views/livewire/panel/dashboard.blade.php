<main class="pb-4">
    <header class="mb-6">
        <h1 class="text-2xl font-semibold tracking-tight">Overview</h1>
        <p class="mt-2 text-sm leading-6 text-slate-600">Signed in as {{ auth()->user()?->name }}.</p>
    </header>

    @if ($snapshot->mode === \App\Enums\AgentMode::Fake)
        <p role="status" class="mb-6 rounded-xl border border-amber-300 bg-amber-50 px-4 py-3 text-sm text-amber-900">
            <strong>Simulated data (fake agent mode).</strong> These values are generated
            locally for development and were not read from this server.
        </p>
    @endif

    @if (! $snapshot->available)
        <p role="status" class="mb-6 rounded-xl border border-slate-300 bg-slate-100 px-4 py-3 text-sm text-slate-700">
            The agent is unavailable. {{ $snapshot->reason }}
        </p>
    @elseif ($snapshot->isStale($staleAfter))
        <p role="status" class="mb-6 rounded-xl border border-amber-300 bg-amber-50 px-4 py-3 text-sm text-amber-900">
            The last observation is stale ({{ $snapshot->sampledAt }}).
        </p>
    @endif

    @php
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
        $sampleHint = $snapshot->available ? 'Sampled '.$snapshot->sampledAt : 'No observation available';
        $uptimeText = $uptime ? floor($uptime['value'] / 86400).'d '.floor(($uptime['value'] % 86400) / 3600).'h' : $dash;
    @endphp

    <section aria-label="Server metrics" class="grid grid-cols-2 gap-4 lg:grid-cols-3">
        <x-ui.stat label="CPU" :value="$cpu ? $cpu['value'] : $dash" unit="{{ $cpu ? '%' : null }}" :hint="$sampleHint" />
        <x-ui.stat label="Memory" :value="$memPercent ?? $dash" unit="{{ $memPercent !== null ? '%' : null }}" :hint="$sampleHint" />
        <x-ui.stat label="Disk" :value="$disk ? $disk['value'] : $dash" unit="{{ $disk ? '%' : null }}" :hint="$sampleHint" />
        <x-ui.stat label="Load (1m)" :value="$load ? $load['value'] : $dash" :hint="$sampleHint" />
        <x-ui.stat label="Uptime" :value="$uptimeText" :hint="$sampleHint" />
        <x-ui.stat label="Active tasks" :value="$activeTasks" hint="Pending, queued or running" />
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
                <div class="flex justify-between gap-4">
                    <dt class="text-slate-600">Agent mode</dt>
                    <dd class="font-medium">{{ $snapshot->mode->value }}</dd>
                </div>
            </dl>
        </x-ui.panel>

        <x-ui.panel title="Managed services">
            @if ($snapshot->services === [])
                <p class="mt-4 text-sm text-slate-600">No services reported by the agent.</p>
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

    <section class="mt-6">
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
