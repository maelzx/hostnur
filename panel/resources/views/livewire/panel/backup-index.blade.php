<main>
    <header class="mb-6">
        <h1 class="text-2xl font-semibold tracking-tight">Backups</h1>
        <p class="mt-2 text-sm leading-6 text-slate-600">
            Backup policy and simulated requests only. A simulated backup is never
            labelled recoverable, restore is not available, and the panel never
            exports its own keys or plaintext secrets.
        </p>
    </header>

    <x-ui.panel title="New backup policy" class="mb-6">
        <form wire:submit="createPolicy" class="grid gap-4 sm:grid-cols-2">
            <div>
                <label for="scope" class="block text-sm font-medium text-slate-700">Scope</label>
                <select id="scope" wire:model.live="scope" class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm">
                    <option value="application">application</option>
                    <option value="server">server</option>
                </select>
                @error('scope') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
            </div>
            <div>
                <label for="backup_application" class="block text-sm font-medium text-slate-700">Application</label>
                <select id="backup_application" wire:model="applicationId" @if ($scope !== 'application') disabled @endif class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm disabled:bg-slate-100">
                    <option value="">Select an application</option>
                    @foreach ($applications as $application)
                        <option value="{{ $application->id }}">{{ $application->name }}</option>
                    @endforeach
                </select>
                @error('application_id') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
            </div>
            <div>
                <label for="backup_expression" class="block text-sm font-medium text-slate-700">Cron expression</label>
                <input id="backup_expression" type="text" wire:model="expression" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 font-mono text-sm">
                @error('expression') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
            </div>
            <div>
                <label for="backup_timezone" class="block text-sm font-medium text-slate-700">Timezone</label>
                <input id="backup_timezone" type="text" wire:model="timezone" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm">
                @error('timezone') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
            </div>
            <div>
                <label for="retention_days" class="block text-sm font-medium text-slate-700">Retention (days)</label>
                <input id="retention_days" type="number" min="1" max="3650" wire:model="retention_days" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm">
                @error('retention_days') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
            </div>
            <div>
                <label for="destination" class="block text-sm font-medium text-slate-700">Destination credential</label>
                <select id="destination" wire:model="destination_credential_id" class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm">
                    <option value="">None</option>
                    @foreach ($credentials as $credential)
                        <option value="{{ $credential->id }}">{{ $credential->label }}</option>
                    @endforeach
                </select>
                @error('destination_credential_id') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
            </div>
            <div class="flex items-center gap-3 sm:col-span-2">
                <label class="inline-flex items-center gap-2 text-sm text-slate-700"><input type="checkbox" wire:model="enabled" class="rounded border-slate-300"> Enabled</label>
                <x-ui.button type="submit">Save policy</x-ui.button>
            </div>
        </form>
    </x-ui.panel>

    <div class="grid gap-6 lg:grid-cols-2">
        <x-ui.panel title="Policies">
            @if ($policies->isEmpty())
                <p class="mt-4 text-sm text-slate-600">No backup policies.</p>
            @else
                <ul class="mt-4 divide-y divide-slate-100 text-sm">
                    @foreach ($policies as $policy)
                        <li class="flex items-center justify-between gap-3 py-2" wire:key="policy-{{ $policy->id }}">
                            <span>
                                <span class="font-medium">{{ $policy->scope->value }}</span>
                                <span class="text-slate-500">{{ $policy->application?->name ?? 'server' }} · {{ $policy->expression }} · {{ $policy->retention_days }}d</span>
                            </span>
                            <span class="flex gap-2">
                                <x-ui.button type="button" variant="secondary" wire:click="requestBackup('{{ $policy->id }}')">Back up now</x-ui.button>
                                <x-ui.button type="button" variant="danger" wire:click="deletePolicy('{{ $policy->id }}')">Delete</x-ui.button>
                            </span>
                        </li>
                    @endforeach
                </ul>
            @endif
        </x-ui.panel>

        <x-ui.panel title="Backups">
            @if ($backups->isEmpty())
                <p class="mt-4 text-sm text-slate-600">No backups recorded.</p>
            @else
                <ul class="mt-4 divide-y divide-slate-100 text-sm">
                    @foreach ($backups as $backup)
                        <li class="flex items-center justify-between gap-3 py-2" wire:key="backup-{{ $backup->id }}">
                            <span>
                                <x-ui.request-status :state="$backup->state" :mode="$backup->isSimulated() ? 'fake' : null" />
                                <span class="ml-2 text-xs text-slate-500">verification: {{ $backup->verification_status }}</span>
                            </span>
                            @if ($backup->task)
                                <a href="{{ route('panel.tasks.show', ['task' => $backup->task->id]) }}" class="text-xs font-medium text-indigo-700 hover:underline">task</a>
                            @endif
                        </li>
                    @endforeach
                </ul>
            @endif
            <p class="mt-4 text-xs text-slate-500">
                Restore is unavailable in this phase; consistency, key custody and
                cross-VM verification arrive later.
            </p>
        </x-ui.panel>
    </div>
</main>
