<main>
    <header class="mb-6">
        <p class="text-sm text-slate-500">
            <a href="{{ route('panel.audit.index') }}" class="font-medium text-indigo-700 hover:underline">Audit history</a>
        </p>
        <h1 class="mt-2 break-words text-2xl font-semibold tracking-tight">{{ $event->action->value }}</h1>
        <p class="mt-2 text-sm text-slate-600">{{ $event->recorded_at?->toDayDateTimeString() ?? '—' }}</p>
    </header>

    <x-ui.panel title="Event detail">
        <dl class="grid gap-4 sm:grid-cols-2">
            <div>
                <dt class="text-sm text-slate-600">Actor</dt>
                <dd class="mt-1 font-medium">{{ $event->actor_label ?? '—' }}</dd>
            </div>
            <div>
                <dt class="text-sm text-slate-600">Outcome</dt>
                <dd class="mt-1"><x-ui.badge :tone="$event->outcome->value === 'failure' ? 'danger' : ($event->outcome->value === 'denied' ? 'warning' : 'success')">{{ $event->outcome->value }}</x-ui.badge></dd>
            </div>
            <div>
                <dt class="text-sm text-slate-600">Resource</dt>
                <dd class="mt-1 break-all font-medium">{{ $event->resource_kind !== null ? $event->resource_kind.' '.$event->resource_id : '—' }}</dd>
            </div>
            <div>
                <dt class="text-sm text-slate-600">Task</dt>
                <dd class="mt-1 break-all font-medium">
                    @if ($event->task_id !== null)
                        <a href="{{ route('panel.tasks.show', ['task' => $event->task_id]) }}" class="text-indigo-700 hover:underline">{{ $event->task_id }}</a>
                    @else
                        —
                    @endif
                </dd>
            </div>
        </dl>

        <h2 class="mt-6 text-sm font-semibold uppercase tracking-wide text-slate-500">Metadata</h2>
        @if (empty($event->metadata))
            <p class="mt-2 text-sm text-slate-600">No additional metadata.</p>
        @else
            <dl class="mt-2 space-y-1 text-sm">
                @foreach ($event->metadata as $key => $value)
                    <div class="flex justify-between gap-4">
                        <dt class="text-slate-600">{{ $key }}</dt>
                        <dd class="break-all font-medium">{{ is_scalar($value) ? $value : json_encode($value) }}</dd>
                    </div>
                @endforeach
            </dl>
        @endif
    </x-ui.panel>
</main>
