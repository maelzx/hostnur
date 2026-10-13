<main class="mx-auto flex min-h-screen max-w-5xl flex-col px-6 py-12 sm:px-10">
    <header class="mb-8">
        <p class="mb-3 text-sm font-semibold uppercase tracking-widest text-indigo-700">{{ config('app.name') }}</p>
        <h1 class="text-2xl font-semibold tracking-tight">Task history</h1>
        <p class="mt-2 text-sm leading-6 text-slate-600">
            Durable, read-only record of requested and executed panel operations. This
            view does not perform privileged actions.
        </p>
    </header>

    @if ($tasks->isEmpty())
        <section class="rounded-2xl border border-slate-200 bg-white p-6 text-sm text-slate-600">
            No tasks have been requested yet.
        </section>
    @else
        <section class="overflow-hidden rounded-2xl border border-slate-200 bg-white">
            <table class="min-w-full divide-y divide-slate-200 text-sm">
                <thead class="bg-slate-50 text-left text-xs uppercase tracking-wide text-slate-500">
                    <tr>
                        <th class="px-4 py-3 font-medium">Operation</th>
                        <th class="px-4 py-3 font-medium">Status</th>
                        <th class="px-4 py-3 font-medium">Resource</th>
                        <th class="px-4 py-3 font-medium">Requested</th>
                    </tr>
                </thead>
                <tbody class="divide-y divide-slate-100">
                    @foreach ($tasks as $task)
                        <tr>
                            <td class="px-4 py-3">
                                <a href="{{ route('panel.tasks.show', ['task' => $task->id]) }}" class="font-medium text-indigo-700 hover:underline">
                                    {{ $task->operation }}
                                </a>
                                <span class="block text-xs text-slate-500">{{ $task->mode }} mode</span>
                            </td>
                            <td class="px-4 py-3">
                                <span class="rounded-full bg-slate-100 px-2 py-1 text-xs font-medium text-slate-700">{{ $task->status->label() }}</span>
                            </td>
                            <td class="px-4 py-3 text-slate-600">
                                {{ $task->resource_kind !== null ? $task->resource_kind.' '.$task->resource_id : '—' }}
                            </td>
                            <td class="px-4 py-3 text-slate-600">{{ $task->requested_at?->toDayDateTimeString() ?? '—' }}</td>
                        </tr>
                    @endforeach
                </tbody>
            </table>
        </section>

        <div class="mt-6">
            {{ $tasks->links() }}
        </div>
    @endif

    <p class="mt-6 text-sm">
        <a href="{{ route('panel.dashboard') }}" class="font-medium text-indigo-700 hover:underline">Back to panel</a>
        ·
        <a href="{{ route('panel.audit.index') }}" class="font-medium text-indigo-700 hover:underline">Audit history</a>
    </p>
</main>
