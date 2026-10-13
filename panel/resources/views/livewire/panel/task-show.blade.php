<main @if ($pollWhileActive) wire:poll.5s @endif>
    <header class="mb-6">
        <p class="text-sm text-slate-500">
            <a href="{{ route('panel.tasks.index') }}" class="font-medium text-indigo-700 hover:underline">Task history</a>
        </p>
        <h1 class="mt-2 break-words text-2xl font-semibold tracking-tight">{{ $task->operation }}</h1>
        <p class="mt-2 flex flex-wrap items-center gap-2 text-sm text-slate-600">
            <x-ui.task-status :status="$task->status" />
            <span>{{ $task->mode }} mode</span>
            @if ($pollWhileActive)
                <span role="status" class="text-slate-500">Polling while active…</span>
            @endif
        </p>
    </header>

    <x-ui.panel class="mb-6">
        <dl class="grid gap-4 sm:grid-cols-2">
            <div>
                <dt class="text-sm text-slate-600">Requested by</dt>
                <dd class="mt-1 font-medium">{{ $task->actor_label ?? '—' }}</dd>
            </div>
            <div>
                <dt class="text-sm text-slate-600">Resource</dt>
                <dd class="mt-1 break-all font-medium">{{ $task->resource_kind !== null ? $task->resource_kind.' '.$task->resource_id : '—' }}</dd>
            </div>
            <div>
                <dt class="text-sm text-slate-600">Requested at</dt>
                <dd class="mt-1 font-medium">{{ $task->requested_at?->toDayDateTimeString() ?? '—' }}</dd>
            </div>
            <div>
                <dt class="text-sm text-slate-600">Finished at</dt>
                <dd class="mt-1 font-medium">{{ $task->finished_at?->toDayDateTimeString() ?? '—' }}</dd>
            </div>
            @if ($task->error_code !== null)
                <div class="sm:col-span-2">
                    <dt class="text-sm text-slate-600">Error</dt>
                    <dd class="mt-1 font-medium text-red-700">{{ $task->error_code }}</dd>
                    @if ($task->error_message)
                        <dd class="mt-1 whitespace-pre-wrap break-words text-sm text-slate-700">{{ $task->error_message }}</dd>
                    @endif
                </div>
            @endif
        </dl>
    </x-ui.panel>

    <x-ui.panel :title="'Events ('.$events->count().' of '.$totalEvents.')'">
        @if ($events->isEmpty())
            <p class="mt-4 text-sm text-slate-600">No events recorded.</p>
        @else
            <ol class="mt-4 space-y-3">
                @foreach ($events as $event)
                    <li class="border-l-2 border-slate-200 pl-4" wire:key="event-{{ $event->sequence }}">
                        <p class="whitespace-pre-wrap break-words text-sm font-medium text-slate-800">{{ $event->message }}</p>
                        <p class="mt-0.5 text-xs text-slate-500">
                            #{{ $event->sequence }} · {{ $event->recorded_at?->toDayDateTimeString() }} · {{ $event->type }} ·
                            <x-ui.badge :tone="$event->level->value === 'error' ? 'danger' : ($event->level->value === 'warning' ? 'warning' : 'neutral')">{{ $event->level->value }}</x-ui.badge>
                        </p>
                    </li>
                @endforeach
            </ol>
        @endif

        @if ($hasMoreEvents)
            <x-ui.button variant="secondary" class="mt-4" wire:click="loadMoreEvents">
                Load more events
            </x-ui.button>
        @endif
    </x-ui.panel>
</main>
