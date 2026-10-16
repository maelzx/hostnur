@php
    $primaryDomain = $domains->first();
    $primaryUrl = $primaryDomain !== null
        ? ($primaryDomain->https_policy->value === 'disabled' ? 'http://' : 'https://').$primaryDomain->hostname
        : null;
    $certsByDomain = $certificates->keyBy('application_domain_id');
@endphp

<main>
    <header class="mb-6">
        <p class="text-sm text-slate-500">
            <a href="{{ route('panel.applications.index') }}" class="font-medium text-indigo-700 hover:underline">Applications</a>
        </p>

        <div class="mt-2 flex flex-wrap items-start justify-between gap-4">
            <div class="min-w-0">
                <h1 class="break-words text-2xl font-semibold tracking-tight text-slate-900">{{ $application->name }}</h1>
                <p class="mt-2 flex flex-wrap items-center gap-2 text-sm text-slate-600">
                    <span class="font-mono">{{ $application->slug }}</span>
                    <x-ui.application-status :status="$application->status()" />
                    <span>{{ $application->provider }}</span>
                    <span>{{ $application->runtime }} {{ $application->runtime_version }}</span>
                </p>
            </div>

            <div class="flex flex-wrap items-center gap-2">
                <a
                    href="{{ route('panel.applications.operations', ['application' => $application->id]) }}"
                    class="inline-flex items-center justify-center gap-2 rounded-lg bg-indigo-600 px-4 py-2 text-sm font-semibold text-white shadow-xs transition-colors hover:bg-indigo-700 focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-2"
                >
                    Deploy
                </a>

                @if ($locked)
                    <x-ui.confirm title="Restart this application?" confirm-label="Restart" method="restart" variant="secondary">
                        <x-slot:trigger>Restart</x-slot:trigger>
                        Reloads the application processes. Recorded as a task.
                    </x-ui.confirm>
                @endif
            </div>
        </div>

        @error('restart') <p role="alert" class="mt-3 text-sm text-red-700">{{ $message }}</p> @enderror

        @if ($primaryUrl !== null)
            <div x-data="{ copied: false }" class="mt-4 flex flex-wrap items-center gap-2">
                <a href="{{ $primaryUrl }}" target="_blank" rel="noopener" class="font-mono text-sm font-medium text-indigo-700 hover:underline">{{ $primaryUrl }}</a>
                <button
                    type="button"
                    x-on:click="navigator.clipboard?.writeText('{{ $primaryUrl }}').then(() => { copied = true; setTimeout(() => copied = false, 2000) })"
                    class="inline-flex items-center gap-1.5 rounded-lg border border-slate-300 bg-white px-2.5 py-1 text-xs font-medium text-slate-600 transition-colors hover:bg-slate-50 focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500"
                >
                    <span x-show="! copied">Copy</span>
                    <span x-show="copied" x-cloak class="text-emerald-700">Copied</span>
                </button>
                <a href="{{ $primaryUrl }}" target="_blank" rel="noopener" class="inline-flex items-center gap-1 text-xs font-medium text-slate-600 hover:text-slate-900">
                    Open
                    <svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><path d="M15 3h6v6"/><path d="M10 14 21 3"/></svg>
                </a>
            </div>
        @endif
    </header>

    @if (session('status'))
        <div class="mb-6 rounded-lg border border-green-200 bg-green-50 px-4 py-3 text-sm text-green-800">{{ session('status') }}</div>
    @endif

    {{-- Overview --}}
    <div class="mb-6 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        <x-ui.stat
            label="Last deploy"
            :value="$latestDeployment?->finished_at?->diffForHumans() ?? ($latestDeployment?->requested_at?->diffForHumans() ?? '—')"
            :hint="$latestDeployment?->state->value ?? 'Not deployed'"
            :tone="$latestDeployment?->state === \App\Enums\DeploymentState::Failed ? 'warning' : 'neutral'"
        />
        <x-ui.stat label="Runtime" :value="$application->runtime" :unit="$application->runtime_version" />
        <x-ui.stat label="Domains" :value="$domains->count()" />
        <x-ui.stat
            label="Database"
            :value="$database !== null ? $database->engine->value : '—'"
            :hint="$database?->database_name ?? 'Not provisioned'"
        />
        <x-ui.stat label="Queue workers" :value="$workers" :hint="$scheduler ? 'Scheduler on' : 'Scheduler off'" />
        <x-ui.stat label="Mode" :value="config('hostnur.agent_mode')" />
    </div>

    {{-- Setup checklist: shown only until the first successful deployment --}}
    @if (! $locked && ! $application->isArchived())
        <x-ui.panel title="Finish setup" class="mb-6">
            <p class="mt-2 text-sm text-slate-600">Complete these, then deploy. Only source and runtime are required.</p>
            <ol class="mt-4 grid gap-2 text-sm sm:grid-cols-2">
                @php
                    $checks = [
                        ['Repository saved', trim($repository_url) !== ''],
                        ['Runtime configured', trim($runtime) !== ''],
                        ['Domain added (recommended)', $domains->isNotEmpty()],
                        ['Secrets added (optional)', $secrets->isNotEmpty()],
                    ];
                @endphp
                @foreach ($checks as [$label, $done])
                    <li class="flex items-center gap-2">
                        <span class="inline-flex h-5 w-5 items-center justify-center rounded-full text-xs font-bold {{ $done ? 'bg-green-100 text-green-700' : 'bg-slate-100 text-slate-400' }}">{{ $done ? '✓' : '•' }}</span>
                        <span class="{{ $done ? 'text-slate-700' : 'text-slate-500' }}">{{ $label }}</span>
                    </li>
                @endforeach
            </ol>
            <div class="mt-4">
                <a href="{{ route('panel.applications.operations', ['application' => $application->id]) }}" class="inline-flex items-center justify-center gap-2 rounded-lg bg-indigo-600 px-4 py-2 text-sm font-semibold text-white shadow-xs transition-colors hover:bg-indigo-700">Go to deploy</a>
            </div>
        </x-ui.panel>
    @endif

    @if ($application->isArchived())
        <x-ui.panel class="mb-6">
            <p class="text-sm text-slate-600">This application is archived. Its configuration and history are read-only.</p>
        </x-ui.panel>
    @endif

    <div class="space-y-6">
        {{-- Configuration --}}
        <x-ui.panel title="Configuration">
            <form wire:submit="saveSource" class="mt-4 space-y-3">
                <div>
                    <label for="repository_url" class="block text-sm font-medium text-slate-700">Repository URL</label>
                    <input id="repository_url" type="text" wire:model="repository_url" @disabled($locked) placeholder="https://github.com/org/repo.git" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm disabled:cursor-not-allowed disabled:bg-slate-100 disabled:text-slate-500 focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                    @if ($locked)
                        <p class="mt-1 text-xs text-slate-500">Locked after the first successful deployment. Create a new application to use a different repository.</p>
                    @else
                        <p class="mt-1 text-xs text-slate-500">Use the HTTPS clone URL, ending in <code>.git</code>.</p>
                    @endif
                    @error('repository_url') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                </div>
                <div class="grid gap-3 sm:grid-cols-2">
                    <div>
                        <label for="git_ref" class="block text-sm font-medium text-slate-700">Branch, tag, or commit</label>
                        <input id="git_ref" type="text" wire:model="git_ref" placeholder="main" @disabled($application->isArchived()) class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm disabled:bg-slate-100">
                        <p class="mt-1 text-xs text-slate-500">Deploys this ref. Leave blank for <code>main</code>.</p>
                        @error('git_ref') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                    </div>
                    <div>
                        <label for="source_credential_id" class="block text-sm font-medium text-slate-700">Credential</label>
                        <select id="source_credential_id" wire:model="source_credential_id" @disabled($application->isArchived()) class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm disabled:bg-slate-100">
                            <option value="">None (public)</option>
                            @foreach ($credentials as $credential)
                                <option value="{{ $credential->id }}">{{ $credential->label }} ({{ $credential->type->value }} v{{ $credential->version }})</option>
                            @endforeach
                        </select>
                        <p class="mt-1 text-xs text-slate-500">For private repositories only.</p>
                        @error('source_credential_id') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                    </div>
                </div>
                <x-ui.button type="submit" variant="secondary">Save source</x-ui.button>
            </form>

            <hr class="my-6 border-slate-200">

            <form wire:submit="saveConfiguration" class="space-y-4">
                <div class="grid gap-4 sm:grid-cols-2">
                    <div>
                        <label for="runtime" class="block text-sm font-medium text-slate-700">Runtime</label>
                        <select id="runtime" wire:model="runtime" class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                            @foreach ($runtimes as $option)<option value="{{ $option }}">{{ $option }}</option>@endforeach
                        </select>
                        @error('runtime') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                    </div>
                    <div>
                        <label for="runtime_version" class="block text-sm font-medium text-slate-700">Runtime version</label>
                        <input id="runtime_version" type="text" wire:model="runtime_version" placeholder="8.3" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                        <p class="mt-1 text-xs text-slate-500">Server-supported version, e.g. 8.3 or 8.4.</p>
                        @error('runtime_version') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                    </div>
                    <div>
                        <label for="workers" class="block text-sm font-medium text-slate-700">Queue workers</label>
                        <input id="workers" type="number" min="0" max="10" wire:model="workers" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                        <p class="mt-1 text-xs text-slate-500">Set to <code>1</code> if the app queues email.</p>
                        @error('workers') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                    </div>
                    <div>
                        <label for="health_path" class="block text-sm font-medium text-slate-700">Health check path</label>
                        <input id="health_path" type="text" wire:model="health_path" placeholder="up" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                        <p class="mt-1 text-xs text-slate-500">Must return 200. Laravel default: <code>/up</code>.</p>
                        @error('health_path') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                    </div>
                </div>

                <label class="flex items-center gap-2 text-sm text-slate-700">
                    <input type="checkbox" wire:model="scheduler" class="rounded border-slate-300">
                    Run the Laravel scheduler
                </label>

                <details class="rounded-lg border border-slate-200 p-3">
                    <summary class="cursor-pointer text-sm font-medium text-slate-700">Advanced (build/start commands, persistent storage)</summary>
                    <div class="mt-3 grid gap-4 sm:grid-cols-2">
                        <div>
                            <label for="build_command" class="block text-sm font-medium text-slate-700">Extra build command</label>
                            <input id="build_command" type="text" wire:model="build_command" placeholder="(handled automatically)" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                            @error('build_command') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                        </div>
                        <div>
                            <label for="start_command" class="block text-sm font-medium text-slate-700">Start command</label>
                            <input id="start_command" type="text" wire:model="start_command" placeholder="(blank — served by PHP-FPM)" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                            @error('start_command') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                        </div>
                        <div class="sm:col-span-2">
                            <label for="persistent_paths_text" class="block text-sm font-medium text-slate-700">Persistent paths (one per line)</label>
                            <textarea id="persistent_paths_text" wire:model="persistent_paths_text" rows="3" placeholder="storage/app" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30"></textarea>
                            <p class="mt-1 text-xs text-slate-500">Kept across deploys, e.g. <code>storage/app</code>.</p>
                            @error('persistent_paths') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                            @error('persistent_paths.*') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                        </div>
                    </div>
                </details>

                <div class="flex flex-wrap items-center gap-3">
                    <x-ui.button type="button" variant="secondary" wire:click="applyLaravelDefaults">Use recommended Laravel settings</x-ui.button>
                    <x-ui.button type="submit">Save configuration</x-ui.button>
                </div>
            </form>

            <details class="mt-6 rounded-lg border border-slate-200 p-3">
                <summary class="cursor-pointer text-sm font-medium text-slate-700">Source credentials</summary>
                <form wire:submit="storeCredential" class="mt-3 space-y-3">
                    <div class="grid gap-3 sm:grid-cols-2">
                        <div>
                            <label for="credential_label" class="block text-sm font-medium text-slate-700">Label</label>
                            <input id="credential_label" type="text" wire:model="credential_label" placeholder="GitHub read-only token" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                            @error('credential_label') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                        </div>
                        <div>
                            <label for="credential_type" class="block text-sm font-medium text-slate-700">Type</label>
                            <select id="credential_type" wire:model="credential_type" class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                                <option value="https_token">HTTPS token</option>
                                <option value="ssh_deploy_key">SSH deploy key</option>
                            </select>
                        </div>
                    </div>
                    <div>
                        <label for="credential_secret" class="block text-sm font-medium text-slate-700">Secret (stored encrypted)</label>
                        <input id="credential_secret" type="password" wire:model="credential_secret" autocomplete="off" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
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
            </details>
        </x-ui.panel>

        {{-- Domains & TLS --}}
        <x-ui.panel title="Domains & HTTPS">
            <form wire:submit="addDomain" class="mt-4 grid gap-3 sm:grid-cols-2">
                <div>
                    <label for="hostname" class="block text-sm font-medium text-slate-700">Hostname</label>
                    <input id="hostname" type="text" wire:model="hostname" placeholder="app.example.com" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                    @error('hostname') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                </div>
                <div>
                    <label for="https_policy" class="block text-sm font-medium text-slate-700">HTTPS</label>
                    <select id="https_policy" wire:model="https_policy" class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                        @foreach ($policies as $policy)<option value="{{ $policy->value }}">{{ $policy->value }}</option>@endforeach
                    </select>
                </div>
                <div class="sm:col-span-2">
                    <x-ui.button type="submit" variant="secondary">Add domain</x-ui.button>
                </div>
            </form>

            @if ($domains->isEmpty())
                <p class="mt-4 text-sm text-slate-600">No domains configured.</p>
            @else
                <ul class="mt-4 divide-y divide-slate-100 text-sm">
                    @foreach ($domains as $domain)
                        @php $cert = $certsByDomain->get($domain->id); @endphp
                        <li class="flex flex-wrap items-center justify-between gap-3 py-2" wire:key="domain-{{ $domain->id }}">
                            <span class="font-mono font-medium">{{ $domain->hostname }}</span>
                            <div class="flex flex-wrap items-center gap-2">
                                @if ($cert !== null)
                                    <x-ui.badge :tone="$cert->state->value === 'succeeded' ? 'success' : ($cert->state->value === 'failed' ? 'danger' : 'info')">{{ $cert->state->value }}</x-ui.badge>
                                @else
                                    <x-ui.button type="button" variant="secondary" wire:click="requestCertificate('{{ $domain->id }}')">Request certificate</x-ui.button>
                                @endif
                                <span class="text-xs text-slate-500">{{ $domain->https_policy->value }}</span>
                                <x-ui.button type="button" variant="danger" wire:click="removeDomain('{{ $domain->id }}')">Remove</x-ui.button>
                            </div>
                        </li>
                    @endforeach
                </ul>
                <p class="mt-3 text-right text-sm"><a href="{{ route('panel.certificates.index') }}" class="font-medium text-indigo-700 hover:underline">Manage certificates</a></p>
            @endif
        </x-ui.panel>

        {{-- Database --}}
        <x-ui.panel title="Database">
            @if ($database === null)
                <p class="mt-2 text-sm text-slate-600">No managed database provisioned.</p>
                <form wire:submit="requestDatabase" class="mt-4 grid gap-3 sm:grid-cols-3">
                    <div>
                        <label for="database_engine" class="block text-sm font-medium text-slate-700">Engine</label>
                        <select id="database_engine" wire:model="database_engine" class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                            @foreach ($engines as $option)<option value="{{ $option }}">{{ $option }}</option>@endforeach
                        </select>
                        @error('engine') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                    </div>
                    <div>
                        <label for="database_name" class="block text-sm font-medium text-slate-700">Database name</label>
                        <input id="database_name" type="text" wire:model="database_name" placeholder="my_app_db" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 font-mono text-sm">
                        @error('database_name') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                    </div>
                    <div>
                        <label for="database_username" class="block text-sm font-medium text-slate-700">Username</label>
                        <input id="database_username" type="text" wire:model="database_username" placeholder="my_app_user" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 font-mono text-sm">
                        @error('username') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                    </div>
                    <div class="sm:col-span-3">
                        <x-ui.button type="submit" variant="secondary">Provision database</x-ui.button>
                    </div>
                </form>
            @else
                <dl class="mt-2 grid gap-3 text-sm sm:grid-cols-3">
                    <div><dt class="text-slate-600">Engine</dt><dd class="font-medium">{{ $database->engine->value }}</dd></div>
                    <div><dt class="text-slate-600">Database</dt><dd class="font-mono font-medium">{{ $database->database_name }}</dd></div>
                    <div><dt class="text-slate-600">State</dt><dd class="font-medium">{{ $database->state->value }}</dd></div>
                </dl>
                <div class="mt-4 flex flex-wrap gap-2">
                    <x-ui.confirm title="Rotate database credentials?" confirm-label="Rotate" method="rotateDatabase" variant="secondary">
                        <x-slot:trigger>Rotate credentials</x-slot:trigger>
                        A new password is generated; redeploy the application to adopt it.
                    </x-ui.confirm>
                    <x-ui.confirm title="Delete this database?" confirm-label="Delete" method="deleteDatabase" variant="danger">
                        <x-slot:trigger>Delete database</x-slot:trigger>
                        Removes the managed database and its principal. This cannot be undone.
                    </x-ui.confirm>
                </div>
            @endif
        </x-ui.panel>

        {{-- Secrets --}}
        <x-ui.panel title="Environment secrets">
            <form wire:submit="setSecret" class="mt-4 grid gap-3 sm:grid-cols-2">
                <div>
                    <label for="secret_name" class="block text-sm font-medium text-slate-700">Variable name</label>
                    <input id="secret_name" type="text" wire:model="secret_name" placeholder="MAIL_HOST" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                    @error('secret_name') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                </div>
                <div>
                    <label for="secret_value" class="block text-sm font-medium text-slate-700">Value (write-only)</label>
                    <input id="secret_value" type="password" wire:model="secret_value" autocomplete="off" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                    @error('secret_value') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                </div>
                <div class="sm:col-span-2">
                    <x-ui.button type="submit" variant="secondary">Store secret</x-ui.button>
                </div>
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

        {{-- Logs --}}
        <x-ui.panel title="Logs">
            <p class="mt-2 text-sm text-slate-600">Operation and deployment events for this application.</p>
            @if ($logs->isEmpty())
                <p class="mt-4 text-sm text-slate-600">No events recorded yet.</p>
            @else
                <ol class="mt-4 space-y-3">
                    @foreach ($logs as $event)
                        <li class="border-l-2 border-slate-200 pl-4" wire:key="log-{{ $event->id }}">
                            <p class="whitespace-pre-wrap break-words text-sm text-slate-800">{{ $event->message }}</p>
                            <p class="mt-0.5 flex flex-wrap items-center gap-2 text-xs text-slate-500">
                                <span>{{ $event->recorded_at?->toDayDateTimeString() }}</span>
                                @if ($event->task !== null)
                                    <span>·</span>
                                    <span class="font-mono">{{ $event->task->operation }}</span>
                                @endif
                                <x-ui.badge :tone="$event->level->value === 'error' ? 'danger' : ($event->level->value === 'warning' ? 'warning' : 'neutral')">{{ $event->level->value }}</x-ui.badge>
                            </p>
                        </li>
                    @endforeach
                </ol>
            @endif
        </x-ui.panel>
    </div>

    @if (! $application->isArchived())
        <x-ui.panel title="Danger zone" class="mt-6">
            <p class="mt-2 text-sm text-slate-600">
                Keeps records and history; refused while the application has active work.
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
