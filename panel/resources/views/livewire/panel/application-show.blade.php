<main>
    <header class="mb-6">
        <p class="text-sm text-slate-500">
            <a href="{{ route('panel.applications.index') }}" class="font-medium text-indigo-700 hover:underline">Applications</a>
        </p>
        <h1 class="mt-2 break-words text-2xl font-semibold tracking-tight">{{ $application->name }}</h1>
        <p class="mt-2 flex flex-wrap items-center gap-2 text-sm text-slate-600">
            <span class="font-mono">{{ $application->slug }}</span>
            @if ($application->isArchived())
                <x-ui.badge tone="warning">archived</x-ui.badge>
            @else
                <x-ui.badge tone="info">desired</x-ui.badge>
            @endif
            <span>provider {{ $application->provider }}</span>
            <span>revision {{ $application->current_revision }}</span>
        </p>
    </header>

    <div class="grid gap-6 lg:grid-cols-2">
        {{-- Source --}}
        <x-ui.panel title="Source">
            <p class="mt-2 text-sm text-slate-600">
                Source status: <span class="font-medium">{{ $application->source_status->value }}</span>.
                Saving never performs a network check.
            </p>
            <form wire:submit="saveSource" class="mt-4 space-y-3">
                <div>
                    <label for="repository_url" class="block text-sm font-medium text-slate-700">Repository URL</label>
                    <input id="repository_url" type="text" wire:model="repository_url" placeholder="https://github.com/org/repo.git" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm">
                    @error('repository_url') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                </div>
                <div>
                    <label for="git_ref" class="block text-sm font-medium text-slate-700">Branch / tag / commit</label>
                    <input id="git_ref" type="text" wire:model="git_ref" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm">
                    @error('git_ref') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                </div>
                <div>
                    <label for="source_credential_id" class="block text-sm font-medium text-slate-700">Credential</label>
                    <select id="source_credential_id" wire:model="source_credential_id" class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm">
                        <option value="">None (public)</option>
                        @foreach ($credentials as $credential)
                            <option value="{{ $credential->id }}">{{ $credential->label }} ({{ $credential->type->value }} v{{ $credential->version }})</option>
                        @endforeach
                    </select>
                    @error('source_credential_id') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                </div>
                <x-ui.button type="submit" variant="secondary">Save source</x-ui.button>
            </form>
        </x-ui.panel>

        {{-- Credentials --}}
        <x-ui.panel title="Source credentials">
            <form wire:submit="storeCredential" class="mt-4 space-y-3">
                <div class="grid gap-3 sm:grid-cols-2">
                    <div>
                        <label for="credential_label" class="block text-sm font-medium text-slate-700">Label</label>
                        <input id="credential_label" type="text" wire:model="credential_label" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm">
                        @error('credential_label') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                    </div>
                    <div>
                        <label for="credential_type" class="block text-sm font-medium text-slate-700">Type</label>
                        <select id="credential_type" wire:model="credential_type" class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm">
                            <option value="https_token">HTTPS token</option>
                            <option value="ssh_deploy_key">SSH deploy key</option>
                        </select>
                    </div>
                </div>
                <div>
                    <label for="credential_secret" class="block text-sm font-medium text-slate-700">Secret (stored encrypted)</label>
                    <input id="credential_secret" type="password" wire:model="credential_secret" autocomplete="off" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm">
                    @error('credential_secret') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                </div>
                <x-ui.button type="submit" variant="secondary">Store credential</x-ui.button>
            </form>

            @if ($credentials->isNotEmpty())
                <ul class="mt-4 divide-y divide-slate-100 text-sm">
                    @foreach ($credentials as $credential)
                        <li class="py-2" wire:key="cred-{{ $credential->id }}">
                            <div class="flex items-center justify-between gap-3">
                                <span class="font-medium">{{ $credential->label }} <span class="text-xs text-slate-500">{{ $credential->type->value }} v{{ $credential->version }}</span></span>
                                <span class="text-xs text-slate-500">{{ $credential->rotated_at?->toDayDateTimeString() }}</span>
                            </div>
                            <div class="mt-2 flex gap-2">
                                <input type="password" wire:model="rotate_secrets.{{ $credential->id }}" autocomplete="off" placeholder="New secret" class="block w-full rounded-lg border border-slate-300 px-3 py-1.5 text-sm">
                                <x-ui.button type="button" variant="secondary" wire:click="rotateCredential('{{ $credential->id }}')">Rotate</x-ui.button>
                            </div>
                        </li>
                    @endforeach
                </ul>
            @endif
        </x-ui.panel>
    </div>

    {{-- Configuration --}}
    <x-ui.panel title="Configuration (revision {{ $application->current_revision }})" class="mt-6">
        <form wire:submit="saveConfiguration" class="mt-4 grid gap-4 sm:grid-cols-2">
            <div>
                <label for="runtime" class="block text-sm font-medium text-slate-700">Runtime</label>
                <select id="runtime" wire:model="runtime" class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm">
                    @foreach ($runtimes as $option)<option value="{{ $option }}">{{ $option }}</option>@endforeach
                </select>
                @error('runtime') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
            </div>
            <div>
                <label for="runtime_version" class="block text-sm font-medium text-slate-700">Runtime version</label>
                <input id="runtime_version" type="text" wire:model="runtime_version" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm">
                @error('runtime_version') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
            </div>
            <div>
                <label for="build_command" class="block text-sm font-medium text-slate-700">Build command (untrusted app intent)</label>
                <input id="build_command" type="text" wire:model="build_command" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm">
                @error('build_command') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
            </div>
            <div>
                <label for="start_command" class="block text-sm font-medium text-slate-700">Start command (untrusted app intent)</label>
                <input id="start_command" type="text" wire:model="start_command" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm">
                @error('start_command') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
            </div>
            <div>
                <label for="workers" class="block text-sm font-medium text-slate-700">Worker processes</label>
                <input id="workers" type="number" min="0" max="10" wire:model="workers" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm">
                @error('workers') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
            </div>
            <div>
                <label for="health_path" class="block text-sm font-medium text-slate-700">Health check path (relative)</label>
                <input id="health_path" type="text" wire:model="health_path" placeholder="up" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm">
                @error('health_path') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
            </div>
            <div class="sm:col-span-2">
                <label for="persistent_paths_text" class="block text-sm font-medium text-slate-700">Persistent paths (one relative path per line)</label>
                <textarea id="persistent_paths_text" wire:model="persistent_paths_text" rows="3" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm"></textarea>
                @error('persistent_paths') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                @error('persistent_paths.*') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
            </div>
            <div class="flex items-center gap-3 sm:col-span-2">
                <label class="inline-flex items-center gap-2 text-sm text-slate-700">
                    <input type="checkbox" wire:model="scheduler" class="rounded border-slate-300">
                    Run scheduler
                </label>
                <x-ui.button type="submit">Save as new revision</x-ui.button>
            </div>
        </form>

        <h3 class="mt-6 text-sm font-semibold uppercase tracking-wide text-slate-500">Recent revisions</h3>
        <ul class="mt-2 text-sm text-slate-600">
            @foreach ($revisions as $revision)
                <li class="py-1" wire:key="rev-{{ $revision->id }}">Revision {{ $revision->revision }} · {{ $revision->created_at?->toDayDateTimeString() }}</li>
            @endforeach
        </ul>
    </x-ui.panel>

    <div class="mt-6 grid gap-6 lg:grid-cols-2">
        {{-- Secrets --}}
        <x-ui.panel title="Environment secrets">
            <form wire:submit="setSecret" class="mt-4 space-y-3">
                <div>
                    <label for="secret_name" class="block text-sm font-medium text-slate-700">Variable name</label>
                    <input id="secret_name" type="text" wire:model="secret_name" placeholder="APP_KEY" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm">
                    @error('secret_name') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                </div>
                <div>
                    <label for="secret_value" class="block text-sm font-medium text-slate-700">Value (write-only, stored encrypted)</label>
                    <input id="secret_value" type="password" wire:model="secret_value" autocomplete="off" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm">
                    @error('secret_value') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                </div>
                <x-ui.button type="submit" variant="secondary">Store secret</x-ui.button>
            </form>

            @if ($secrets->isEmpty())
                <p class="mt-4 text-sm text-slate-600">No secrets stored.</p>
            @else
                <ul class="mt-4 divide-y divide-slate-100 text-sm">
                    @foreach ($secrets as $secret)
                        <li class="flex items-center justify-between gap-3 py-2" wire:key="secret-{{ $secret->id }}">
                            <span class="font-mono font-medium">{{ $secret->name }}</span>
                            <span class="text-xs text-slate-500">v{{ $secret->version }} · {{ $secret->rotated_at?->toDayDateTimeString() }}</span>
                            <x-ui.button type="button" variant="danger" wire:click="deleteSecret('{{ $secret->name }}')">Delete</x-ui.button>
                        </li>
                    @endforeach
                </ul>
            @endif
        </x-ui.panel>

        {{-- Domains --}}
        <x-ui.panel title="Domains">
            <form wire:submit="addDomain" class="mt-4 space-y-3">
                <div>
                    <label for="hostname" class="block text-sm font-medium text-slate-700">Hostname</label>
                    <input id="hostname" type="text" wire:model="hostname" placeholder="app.example.com" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm">
                    @error('hostname') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                </div>
                <div>
                    <label for="https_policy" class="block text-sm font-medium text-slate-700">HTTPS policy</label>
                    <select id="https_policy" wire:model="https_policy" class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm">
                        @foreach ($policies as $policy)<option value="{{ $policy->value }}">{{ $policy->value }}</option>@endforeach
                    </select>
                </div>
                <x-ui.button type="submit" variant="secondary">Add domain</x-ui.button>
            </form>

            @if ($domains->isEmpty())
                <p class="mt-4 text-sm text-slate-600">No domains configured.</p>
            @else
                <ul class="mt-4 divide-y divide-slate-100 text-sm">
                    @foreach ($domains as $domain)
                        <li class="flex items-center justify-between gap-3 py-2" wire:key="domain-{{ $domain->id }}">
                            <span class="font-mono font-medium">{{ $domain->hostname }}</span>
                            <span class="text-xs text-slate-500">{{ $domain->https_policy->value }}</span>
                            <x-ui.button type="button" variant="danger" wire:click="removeDomain('{{ $domain->id }}')">Remove</x-ui.button>
                        </li>
                    @endforeach
                </ul>
            @endif
        </x-ui.panel>
    </div>

    @if (! $application->isArchived())
        <x-ui.panel title="Archive" class="mt-6">
            <p class="mt-2 text-sm text-slate-600">
                Archiving keeps the application and its history; it does not delete
                records. It is refused while the application has active work.
            </p>
            @error('archive') <p role="alert" class="mt-2 text-sm text-red-700">{{ $message }}</p> @enderror
            <div class="mt-4">
                <x-ui.confirm title="Archive this application?" confirm-label="Archive" method="archive" variant="danger">
                    <x-slot:trigger>Archive application</x-slot:trigger>
                    History and configuration remain stored; no host change is made.
                </x-ui.confirm>
            </div>
        </x-ui.panel>
    @endif
</main>
