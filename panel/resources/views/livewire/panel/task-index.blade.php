<main>
    <header class="mb-6">
        <h1 class="text-2xl font-semibold tracking-tight">Task history</h1>
        <p class="mt-2 text-sm leading-6 text-slate-600">
            Durable, read-only record of requested and executed panel operations.
        </p>
    </header>

    <x-ui.panel class="mb-6">
        <form class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3" role="search">
            <div>
                <label for="status-filter" class="block text-sm font-medium text-slate-700">Status</label>
                <select
                    id="status-filter"
                    wire:model.live="status"
                    class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm focus:border-indigo-500 focus:outline-none focus-visible:ring-1 focus-visible:ring-indigo-500"
                >
                    <option value="">All statuses</option>
                    @foreach ($statuses as $option)
                        <option value="{{ $option->value }}">{{ $option->label() }}</option>
                    @endforeach
                </select>
            </div>
            <div>
                <label for="operation-filter" class="block text-sm font-medium text-slate-700">Operation</label>
                <input
                    id="operation-filter"
                    type="search"
                    wire:model.live.debounce.300ms="operation"
                    placeholder="e.g. application"
                    class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm focus:border-indigo-500 focus:outline-none focus-visible:ring-1 focus-visible:ring-indigo-500"
                >
            </div>
        </form>
    </x-ui.panel>

    @if ($tasks->isEmpty())
        <x-ui.empty-state title="No matching tasks">Adjust the filters or request an operation.</x-ui.empty-state>
    @else
        <section class="overflow-hidden rounded-2xl border border-slate-200 bg-white">
            <div class="overflow-x-auto">
                <table class="min-w-full divide-y divide-slate-200 text-sm">
                    <caption class="sr-only">Requested panel tasks</caption>
                    <thead class="bg-slate-50 text-left text-xs uppercase tracking-wide text-slate-500">
                        <tr>
                            <th scope="col" class="px-4 py-3 font-medium">Operation</th>
                            <th scope="col" class="px-4 py-3 font-medium">Status</th>
                            <th scope="col" class="px-4 py-3 font-medium">Resource</th>
                            <th scope="col" class="px-4 py-3 font-medium">Requested</th>
                        </tr>
                    </thead>
                    <tbody class="divide-y divide-slate-100">
                        @foreach ($tasks as $task)
                            <tr>
                                <th scope="row" class="px-4 py-3 text-left font-normal">
                                    <a href="{{ route('panel.tasks.show', ['task' => $task->id]) }}" class="font-medium text-indigo-700 hover:underline">
                                        {{ $task->operation }}
                                    </a>
                                    <span class="block text-xs text-slate-500">{{ $task->mode }} mode</span>
                                </th>
                                <td class="px-4 py-3"><x-ui.task-status :status="$task->status" /></td>
                                <td class="px-4 py-3 text-slate-600">
                                    {{ $task->resource_kind !== null ? $task->resource_kind.' '.$task->resource_id : '—' }}
                                </td>
                                <td class="px-4 py-3 text-slate-600">{{ $task->requested_at?->toDayDateTimeString() ?? '—' }}</td>
                            </tr>
                        @endforeach
                    </tbody>
                </table>
            </div>
        </section>

        <div class="mt-6">
            {{ $tasks->links() }}
        </div>
    @endif
</main>
