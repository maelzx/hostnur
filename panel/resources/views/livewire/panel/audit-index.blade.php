<main>
    <header class="mb-6">
        <h1 class="text-2xl font-semibold tracking-tight">Audit history</h1>
    </header>

    <x-ui.panel class="mb-6">
        <form class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3" role="search">
            <div>
                <label for="action-filter" class="block text-sm font-medium text-slate-700">Action</label>
                <select
                    id="action-filter"
                    wire:model.live="action"
                    class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm focus:border-indigo-500 focus:outline-none focus-visible:ring-1 focus-visible:ring-indigo-500"
                >
                    <option value="">All actions</option>
                    @foreach ($actions as $option)
                        <option value="{{ $option->value }}">{{ $option->value }}</option>
                    @endforeach
                </select>
            </div>
            <div>
                <label for="actor-filter" class="block text-sm font-medium text-slate-700">Actor</label>
                <input
                    id="actor-filter"
                    type="search"
                    wire:model.live.debounce.300ms="actor"
                    placeholder="email"
                    class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm focus:border-indigo-500 focus:outline-none focus-visible:ring-1 focus-visible:ring-indigo-500"
                >
            </div>
        </form>
    </x-ui.panel>

    @if ($events->isEmpty())
        <x-ui.empty-state title="No matching audit events">Adjust the filters.</x-ui.empty-state>
    @else
        <section class="overflow-hidden rounded-xl border border-slate-200 bg-white shadow-xs">
            <div class="overflow-x-auto">
                <table class="min-w-full divide-y divide-slate-200 text-sm">
                    <caption class="sr-only">Authentication and task audit events</caption>
                    <thead class="bg-slate-50 text-left text-xs uppercase tracking-wide text-slate-500">
                        <tr>
                            <th scope="col" class="px-4 py-3 font-medium">When</th>
                            <th scope="col" class="px-4 py-3 font-medium">Action</th>
                            <th scope="col" class="px-4 py-3 font-medium">Actor</th>
                            <th scope="col" class="px-4 py-3 font-medium">Outcome</th>
                            <th scope="col" class="px-4 py-3 font-medium">Resource</th>
                        </tr>
                    </thead>
                    <tbody class="divide-y divide-slate-100">
                        @foreach ($events as $event)
                            <tr wire:key="audit-{{ $event->id }}">
                                <td class="px-4 py-3 text-slate-600">{{ $event->recorded_at?->toDayDateTimeString() ?? '—' }}</td>
                                <td class="px-4 py-3 font-medium">
                                    <a href="{{ route('panel.audit.show', ['event' => $event->id]) }}" class="text-indigo-700 hover:underline">{{ $event->action->value }}</a>
                                </td>
                                <td class="px-4 py-3 text-slate-600">{{ $event->actor_label ?? '—' }}</td>
                                <td class="px-4 py-3"><x-ui.badge :tone="$event->outcome->value === 'failure' ? 'danger' : ($event->outcome->value === 'denied' ? 'warning' : 'success')">{{ $event->outcome->value }}</x-ui.badge></td>
                                <td class="px-4 py-3 text-slate-600">{{ $event->resource_kind !== null ? $event->resource_kind.' '.$event->resource_id : '—' }}</td>
                            </tr>
                        @endforeach
                    </tbody>
                </table>
            </div>
        </section>

        <div class="mt-6">
            {{ $events->links() }}
        </div>
    @endif
</main>
