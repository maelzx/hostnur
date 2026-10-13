<main class="mx-auto flex min-h-screen max-w-5xl flex-col px-6 py-12 sm:px-10">
    <header class="mb-8">
        <p class="mb-3 text-sm font-semibold uppercase tracking-widest text-indigo-700">{{ config('app.name') }}</p>
        <h1 class="text-2xl font-semibold tracking-tight">Audit history</h1>
        <p class="mt-2 text-sm leading-6 text-slate-600">
            Append-only record of authentication and task activity. Secrets, codes and
            provisioning values are never stored here.
        </p>
    </header>

    @if ($events->isEmpty())
        <section class="rounded-2xl border border-slate-200 bg-white p-6 text-sm text-slate-600">
            No audit events recorded yet.
        </section>
    @else
        <section class="overflow-hidden rounded-2xl border border-slate-200 bg-white">
            <table class="min-w-full divide-y divide-slate-200 text-sm">
                <thead class="bg-slate-50 text-left text-xs uppercase tracking-wide text-slate-500">
                    <tr>
                        <th class="px-4 py-3 font-medium">When</th>
                        <th class="px-4 py-3 font-medium">Action</th>
                        <th class="px-4 py-3 font-medium">Actor</th>
                        <th class="px-4 py-3 font-medium">Outcome</th>
                        <th class="px-4 py-3 font-medium">Resource</th>
                    </tr>
                </thead>
                <tbody class="divide-y divide-slate-100">
                    @foreach ($events as $event)
                        <tr>
                            <td class="px-4 py-3 text-slate-600">{{ $event->recorded_at?->toDayDateTimeString() ?? '—' }}</td>
                            <td class="px-4 py-3 font-medium text-slate-800">{{ $event->action->value }}</td>
                            <td class="px-4 py-3 text-slate-600">{{ $event->actor_label ?? '—' }}</td>
                            <td class="px-4 py-3 text-slate-600">{{ $event->outcome->value }}</td>
                            <td class="px-4 py-3 text-slate-600">{{ $event->resource_kind !== null ? $event->resource_kind.' '.$event->resource_id : '—' }}</td>
                        </tr>
                    @endforeach
                </tbody>
            </table>
        </section>

        <div class="mt-6">
            {{ $events->links() }}
        </div>
    @endif

    <p class="mt-6 text-sm">
        <a href="{{ route('panel.dashboard') }}" class="font-medium text-indigo-700 hover:underline">Back to panel</a>
        ·
        <a href="{{ route('panel.tasks.index') }}" class="font-medium text-indigo-700 hover:underline">Task history</a>
    </p>
</main>
