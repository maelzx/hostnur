<main class="mx-auto flex min-h-screen max-w-3xl flex-col px-6 py-12">
    <header class="mb-8">
        <p class="mb-3 text-sm font-semibold uppercase tracking-widest text-indigo-700">{{ config('app.name') }}</p>
        <h1 class="text-2xl font-semibold tracking-tight">{{ $task->operation }}</h1>
        <p class="mt-2 text-sm text-slate-600">
            <span class="rounded-full bg-slate-100 px-2 py-1 text-xs font-medium text-slate-700">{{ $task->status->label() }}</span>
            · {{ $task->mode }} mode
        </p>
    </header>

    <section class="rounded-2xl border border-slate-200 bg-white p-6">
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
                        <dd class="mt-1 text-sm text-slate-700">{{ $task->error_message }}</dd>
                    @endif
                </div>
            @endif
        </dl>
    </section>

    <section class="mt-6 rounded-2xl border border-slate-200 bg-white p-6">
        <h2 class="text-lg font-semibold">Events</h2>
        <ol class="mt-4 space-y-3">
            @foreach ($task->events as $event)
                <li class="border-l-2 border-slate-200 pl-4">
                    <p class="text-sm font-medium text-slate-800">{{ $event->message }}</p>
                    <p class="text-xs text-slate-500">{{ $event->recorded_at?->toDayDateTimeString() }} · {{ $event->type }} · {{ $event->level->value }}</p>
                </li>
            @endforeach
        </ol>
    </section>

    <p class="mt-6 text-sm">
        <a href="{{ route('panel.tasks.index') }}" class="font-medium text-indigo-700 hover:underline">Back to task history</a>
    </p>
</main>
