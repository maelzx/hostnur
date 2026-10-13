<main @if ($pollWhileActive) wire:poll.5s @endif>
    <header class="mb-6">
        <p class="text-sm text-slate-500">
            <a href="{{ route('panel.deployments.index') }}" class="font-medium text-indigo-700 hover:underline">Deployments</a>
        </p>
        <h1 class="mt-2 break-words text-2xl font-semibold tracking-tight">{{ $deployment->operation }}</h1>
        <p class="mt-2 flex flex-wrap items-center gap-2 text-sm text-slate-600">
            <x-ui.badge :tone="$deployment->state->value === 'failed' ? 'danger' : ($deployment->state->value === 'succeeded' ? 'success' : ($deployment->state->value === 'unknown' ? 'warning' : 'info'))">{{ $deployment->state->label() }}</x-ui.badge>
            @if ($deployment->isSimulated())
                <x-ui.badge tone="warning">simulated</x-ui.badge>
            @else
                <x-ui.badge tone="info">{{ $deployment->mode }}</x-ui.badge>
            @endif
            @if ($deployment->application)
                <a href="{{ route('panel.applications.show', ['application' => $deployment->application->id]) }}" class="font-medium text-indigo-700 hover:underline">{{ $deployment->application->name }}</a>
            @endif
        </p>
    </header>

    @if ($deployment->state === \App\Enums\DeploymentState::Unknown)
        <p role="status" class="mb-6 rounded-2xl border border-amber-300 bg-amber-50 px-4 py-3 text-sm text-amber-900">
            The outcome is unknown and needs investigation. Unknown is not success.
        </p>
    @endif

    @if ($deployment->error_code !== null)
        <p role="alert" class="mb-6 rounded-2xl border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-800">
            {{ $deployment->error_code }}: {{ $deployment->error_message }}
        </p>
    @endif

    <x-ui.panel title="Summary" class="mb-6">
        <dl class="grid gap-4 sm:grid-cols-2">
            <div><dt class="text-sm text-slate-600">Revision</dt><dd class="mt-1 font-medium">{{ $deployment->revision_number ?? '—' }}</dd></div>
            <div><dt class="text-sm text-slate-600">Git ref</dt><dd class="mt-1 break-all font-medium">{{ $deployment->git_ref ?? '—' }}</dd></div>
            <div><dt class="text-sm text-slate-600">Resolved commit</dt><dd class="mt-1 break-all font-medium">{{ $deployment->resolved_commit ?? '—' }}</dd></div>
            <div><dt class="text-sm text-slate-600">Release</dt><dd class="mt-1 break-all font-medium">{{ $deployment->release_reference ?? '—' }}</dd></div>
            <div><dt class="text-sm text-slate-600">Migrations</dt><dd class="mt-1 font-medium">{{ $deployment->run_migrations ? 'requested' : 'not run' }}</dd></div>
            <div><dt class="text-sm text-slate-600">Requested</dt><dd class="mt-1 font-medium">{{ $deployment->requested_at?->toDayDateTimeString() ?? '—' }}</dd></div>
            <div><dt class="text-sm text-slate-600">Finished</dt><dd class="mt-1 font-medium">{{ $deployment->finished_at?->toDayDateTimeString() ?? '—' }}</dd></div>
            <div><dt class="text-sm text-slate-600">Task</dt><dd class="mt-1 font-medium">
                @if ($task)
                    <a href="{{ route('panel.tasks.show', ['task' => $task->id]) }}" class="text-indigo-700 hover:underline">{{ $task->status->label() }}</a>
                @else — @endif
            </dd></div>
        </dl>

        @if ($pollWhileActive)
            <p role="status" class="mt-4 text-sm text-slate-500">Polling while active…</p>
        @endif

        @if ($deployment->state === \App\Enums\DeploymentState::Requested || $deployment->state === \App\Enums\DeploymentState::Running)
            <div class="mt-4">
                <x-ui.button type="button" variant="danger" wire:click="cancel">Request cancellation</x-ui.button>
            </div>
        @endif
    </x-ui.panel>

    <x-ui.panel :title="'Progress log ('.$events->count().' of '.$totalEvents.')'">
        @if ($events->isEmpty())
            <p class="mt-4 text-sm text-slate-600">No progress recorded yet.</p>
        @else
            <ol class="mt-4 space-y-3">
                @foreach ($events as $event)
                    <li class="border-l-2 border-slate-200 pl-4" wire:key="dep-event-{{ $event->sequence }}">
                        <p class="whitespace-pre-wrap break-words text-sm font-medium text-slate-800">{{ $event->message }}</p>
                        <p class="mt-0.5 text-xs text-slate-500">#{{ $event->sequence }} · {{ $event->recorded_at?->toDayDateTimeString() }} · {{ $event->type }}</p>
                    </li>
                @endforeach
            </ol>
        @endif

        @if ($hasMoreEvents)
            <x-ui.button type="button" variant="secondary" class="mt-4" wire:click="loadMoreEvents">Load more</x-ui.button>
        @endif
    </x-ui.panel>
</main>
