<main>
    <header class="mb-6">
        <h1 class="text-2xl font-semibold tracking-tight">Scheduled jobs</h1>
    </header>

    <x-ui.panel :title="$editJobId ? 'Edit schedule' : 'New schedule'" class="mb-6">
        <form wire:submit="save" class="grid gap-4 sm:grid-cols-2">
            <div>
                <label for="schedule_application" class="block text-sm font-medium text-slate-700">Application</label>
                <select id="schedule_application" wire:model="applicationId" class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm">
                    <option value="">Select an application</option>
                    @foreach ($applications as $application)
                        <option value="{{ $application->id }}">{{ $application->name }}</option>
                    @endforeach
                </select>
                @error('application_id') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
            </div>
            <div>
                <label for="schedule_name" class="block text-sm font-medium text-slate-700">Name</label>
                <input id="schedule_name" type="text" wire:model="name" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm">
                @error('name') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
            </div>
            <div>
                <label for="expression" class="block text-sm font-medium text-slate-700">Cron expression</label>
                <input id="expression" type="text" wire:model="expression" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 font-mono text-sm">
                @error('expression') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
            </div>
            <div>
                <label for="timezone" class="block text-sm font-medium text-slate-700">Timezone</label>
                <input id="timezone" type="text" wire:model="timezone" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm">
                @error('timezone') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
            </div>
            <div class="sm:col-span-2">
                <label for="command_reference" class="block text-sm font-medium text-slate-700">Command reference</label>
                <input id="command_reference" type="text" wire:model="command_reference" placeholder="schedule:run" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 font-mono text-sm">
                <p class="mt-1 text-xs text-slate-500">A logical reference for the hosted app; the panel does not execute it.</p>
                @error('command_reference') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
            </div>
            <div class="flex items-center gap-3 sm:col-span-2">
                <label class="inline-flex items-center gap-2 text-sm text-slate-700"><input type="checkbox" wire:model="enabled" class="rounded border-slate-300"> Enabled</label>
                <x-ui.button type="submit">Save schedule</x-ui.button>
            </div>
        </form>
    </x-ui.panel>

    @if ($jobs->isEmpty())
        <x-ui.empty-state title="No schedules yet">Create a schedule above.</x-ui.empty-state>
    @else
        <section class="overflow-hidden rounded-xl border border-slate-200 bg-white shadow-xs">
            <div class="overflow-x-auto">
                <table class="min-w-full divide-y divide-slate-200 text-sm">
                    <caption class="sr-only">Application schedules</caption>
                    <thead class="bg-slate-50 text-left text-xs uppercase tracking-wide text-slate-500">
                        <tr>
                            <th scope="col" class="px-4 py-3 font-medium">Application</th>
                            <th scope="col" class="px-4 py-3 font-medium">Name</th>
                            <th scope="col" class="px-4 py-3 font-medium">Expression</th>
                            <th scope="col" class="px-4 py-3 font-medium">Timezone</th>
                            <th scope="col" class="px-4 py-3 font-medium">Actions</th>
                        </tr>
                    </thead>
                    <tbody class="divide-y divide-slate-100">
                        @foreach ($jobs as $job)
                            <tr wire:key="job-{{ $job->id }}">
                                <td class="px-4 py-3 text-slate-600">{{ $job->application?->name ?? '—' }}</td>
                                <td class="px-4 py-3 font-medium">{{ $job->name }}</td>
                                <td class="px-4 py-3 font-mono text-slate-600">{{ $job->expression }}</td>
                                <td class="px-4 py-3 text-slate-600">{{ $job->timezone }}</td>
                                <td class="px-4 py-3">
                                    <div class="flex flex-wrap gap-2">
                                        <x-ui.button type="button" variant="secondary" wire:click="edit('{{ $job->id }}')">Edit</x-ui.button>
                                        <x-ui.button type="button" variant="secondary" wire:click="apply('{{ $job->id }}')">Apply</x-ui.button>
                                        <x-ui.button type="button" variant="danger" wire:click="delete('{{ $job->id }}')">Delete</x-ui.button>
                                    </div>
                                </td>
                            </tr>
                        @endforeach
                    </tbody>
                </table>
            </div>
        </section>

        <div class="mt-6">{{ $jobs->links() }}</div>
        <p class="mt-4 text-xs text-slate-500">Execution history is produced by the agent in a later phase; no runs are recorded yet.</p>
    @endif
</main>
