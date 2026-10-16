<main class="mx-auto max-w-3xl">
    <header class="mb-6">
        <p class="text-sm text-slate-500">
            <a href="{{ route('panel.applications.index') }}" class="font-medium text-indigo-700 hover:underline">Applications</a>
        </p>
        <h1 class="mt-2 text-2xl font-semibold tracking-tight text-slate-900">New application</h1>
    </header>

    {{-- Step indicator --}}
    <ol class="mb-6 flex flex-wrap items-center gap-x-2 gap-y-2 text-sm">
        @foreach ($steps as $index => $label)
            <li class="flex items-center gap-2">
                <button
                    type="button"
                    @if ($index < $step) wire:click="goTo({{ $index }})" @else disabled @endif
                    @class([
                        'inline-flex items-center gap-2 rounded-full px-3 py-1 font-medium transition-colors',
                        'bg-indigo-600 text-white' => $index === $step,
                        'bg-indigo-50 text-indigo-700 hover:bg-indigo-100' => $index < $step,
                        'text-slate-400' => $index > $step,
                    ])
                >
                    <span @class([
                        'inline-flex h-5 w-5 items-center justify-center rounded-full text-xs',
                        'bg-white/20' => $index === $step,
                        'bg-indigo-100 text-indigo-700' => $index < $step,
                        'bg-slate-100 text-slate-500' => $index > $step,
                    ])>{{ $index + 1 }}</span>
                    {{ $label }}
                </button>
                @if (! $loop->last)
                    <span class="text-slate-300" aria-hidden="true">/</span>
                @endif
            </li>
        @endforeach
    </ol>

    <x-ui.panel>
        {{-- Step 0: Basics --}}
        @if ($step === 0)
            <div class="space-y-4">
                <div>
                    <label for="name" class="block text-sm font-medium text-slate-700">Display name</label>
                    <input id="name" type="text" wire:model="name" placeholder="My application" class="mt-1.5 block w-full rounded-lg border border-slate-300 bg-white px-3.5 py-2.5 text-sm text-slate-900 shadow-xs focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                    @error('name') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                </div>
                <div>
                    <label for="slug" class="block text-sm font-medium text-slate-700">Slug</label>
                    <input id="slug" type="text" wire:model="slug" placeholder="my-app" class="mt-1.5 block w-full rounded-lg border border-slate-300 bg-white px-3.5 py-2.5 font-mono text-sm text-slate-900 shadow-xs focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                    <p class="mt-1 text-xs text-slate-500">Lowercase letters, numbers and hyphens; immutable after creation.</p>
                    @error('slug') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                </div>
                <div class="grid gap-4 sm:grid-cols-2">
                    <div>
                        <label for="provider" class="block text-sm font-medium text-slate-700">Provider</label>
                        <select id="provider" wire:model="provider" class="mt-1.5 block w-full rounded-lg border border-slate-300 bg-white px-3.5 py-2.5 text-sm text-slate-900 shadow-xs focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                            @foreach ($providers as $option)
                                <option value="{{ $option }}">{{ $option }}</option>
                            @endforeach
                        </select>
                    </div>
                    <div>
                        <label for="runtime" class="block text-sm font-medium text-slate-700">Runtime</label>
                        <select id="runtime" wire:model="runtime" class="mt-1.5 block w-full rounded-lg border border-slate-300 bg-white px-3.5 py-2.5 text-sm text-slate-900 shadow-xs focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                            @foreach ($runtimes as $option)
                                <option value="{{ $option }}">{{ $option }}</option>
                            @endforeach
                        </select>
                    </div>
                </div>
                <div>
                    <label for="runtime_version" class="block text-sm font-medium text-slate-700">Runtime version</label>
                    <input id="runtime_version" type="text" wire:model="runtime_version" placeholder="8.3" class="mt-1.5 block w-full rounded-lg border border-slate-300 bg-white px-3.5 py-2.5 text-sm text-slate-900 shadow-xs focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                    <p class="mt-1 text-xs text-slate-500">Server-supported version, e.g. 8.3 or 8.4.</p>
                </div>
            </div>
        @endif

        {{-- Step 1: Source --}}
        @if ($step === 1)
            <div class="space-y-4">
                <div>
                    <label for="repository_url" class="block text-sm font-medium text-slate-700">Repository URL</label>
                    <input id="repository_url" type="text" wire:model="repository_url" placeholder="https://github.com/org/repo.git" class="mt-1.5 block w-full rounded-lg border border-slate-300 bg-white px-3.5 py-2.5 text-sm text-slate-900 shadow-xs focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                    <p class="mt-1 text-xs text-slate-500">Optional — leave blank to configure the source later.</p>
                    @error('repository_url') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                </div>
                <div class="grid gap-4 sm:grid-cols-2">
                    <div>
                        <label for="git_ref" class="block text-sm font-medium text-slate-700">Git ref</label>
                        <input id="git_ref" type="text" wire:model="git_ref" placeholder="main" class="mt-1.5 block w-full rounded-lg border border-slate-300 bg-white px-3.5 py-2.5 text-sm text-slate-900 shadow-xs focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                        <p class="mt-1 text-xs text-slate-500">Branch, tag or commit. Defaults to <code>main</code>.</p>
                        @error('git_ref') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                    </div>
                    <div>
                        <label for="source_credential_id" class="block text-sm font-medium text-slate-700">Credential</label>
                        <select id="source_credential_id" wire:model="source_credential_id" class="mt-1.5 block w-full rounded-lg border border-slate-300 bg-white px-3.5 py-2.5 text-sm text-slate-900 shadow-xs focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                            <option value="">None (public)</option>
                            @foreach ($credentials as $credential)
                                <option value="{{ $credential->getKey() }}">{{ $credential->label }}</option>
                            @endforeach
                        </select>
                        <p class="mt-1 text-xs text-slate-500">For private repositories only.</p>
                        @error('source_credential_id') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                    </div>
                </div>
            </div>
        @endif

        {{-- Step 2: Runtime & build --}}
        @if ($step === 2)
            <div class="space-y-4">
                <div class="flex items-center justify-between gap-3">
                    <p class="text-sm text-slate-600">Optional. Use recommended settings for a standard Laravel app.</p>
                    <x-ui.button type="button" variant="secondary" wire:click="applyLaravelDefaults">Use recommended Laravel settings</x-ui.button>
                </div>
                <div class="grid gap-4 sm:grid-cols-2">
                    <div>
                        <label for="build_command" class="block text-sm font-medium text-slate-700">Build command</label>
                        <input id="build_command" type="text" wire:model="build_command" class="mt-1.5 block w-full rounded-lg border border-slate-300 bg-white px-3.5 py-2.5 font-mono text-sm text-slate-900 shadow-xs focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                        <p class="mt-1 text-xs text-slate-500">Runs automatically when detected.</p>
                    </div>
                    <div>
                        <label for="start_command" class="block text-sm font-medium text-slate-700">Start command</label>
                        <input id="start_command" type="text" wire:model="start_command" class="mt-1.5 block w-full rounded-lg border border-slate-300 bg-white px-3.5 py-2.5 font-mono text-sm text-slate-900 shadow-xs focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                    </div>
                </div>
                <div class="grid gap-4 sm:grid-cols-2">
                    <div>
                        <label for="health_path" class="block text-sm font-medium text-slate-700">Health check path</label>
                        <input id="health_path" type="text" wire:model="health_path" placeholder="up" class="mt-1.5 block w-full rounded-lg border border-slate-300 bg-white px-3.5 py-2.5 text-sm text-slate-900 shadow-xs focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                        <p class="mt-1 text-xs text-slate-500">Must return 200. Laravel default: <code>/up</code>.</p>
                    </div>
                    <div>
                        <label for="workers" class="block text-sm font-medium text-slate-700">Queue workers</label>
                        <input id="workers" type="number" min="0" max="10" wire:model="workers" class="mt-1.5 block w-full rounded-lg border border-slate-300 bg-white px-3.5 py-2.5 text-sm text-slate-900 shadow-xs focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                        <p class="mt-1 text-xs text-slate-500">Set to <code>1</code> if the app queues email.</p>
                    </div>
                </div>
                <div>
                    <label for="persistent_paths_text" class="block text-sm font-medium text-slate-700">Persistent paths</label>
                    <textarea id="persistent_paths_text" wire:model="persistent_paths_text" rows="2" class="mt-1.5 block w-full rounded-lg border border-slate-300 bg-white px-3.5 py-2.5 font-mono text-sm text-slate-900 shadow-xs focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30"></textarea>
                    <p class="mt-1 text-xs text-slate-500">One per line; kept across deploys, e.g. <code>storage/app</code>.</p>
                </div>
                <label class="inline-flex items-center gap-2 text-sm text-slate-700">
                    <input type="checkbox" wire:model="scheduler" class="rounded border-slate-300">
                    Run the Laravel scheduler
                </label>
            </div>
        @endif

        {{-- Step 3: Domain & TLS --}}
        @if ($step === 3)
            <div class="space-y-4">
                <div>
                    <label for="hostname" class="block text-sm font-medium text-slate-700">Domain</label>
                    <input id="hostname" type="text" wire:model="hostname" placeholder="app.example.com" class="mt-1.5 block w-full rounded-lg border border-slate-300 bg-white px-3.5 py-2.5 text-sm text-slate-900 shadow-xs focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                    <p class="mt-1 text-xs text-slate-500">Optional — point the domain’s A record here first.</p>
                    @error('hostname') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                </div>
                <div>
                    <label for="https_policy" class="block text-sm font-medium text-slate-700">HTTPS</label>
                    <select id="https_policy" wire:model="https_policy" class="mt-1.5 block w-full rounded-lg border border-slate-300 bg-white px-3.5 py-2.5 text-sm text-slate-900 shadow-xs focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                        @foreach ($policies as $policy)
                            <option value="{{ $policy->value }}">{{ $policy->value }}</option>
                        @endforeach
                    </select>
                </div>
                <label class="inline-flex items-center gap-2 text-sm text-slate-700">
                    <input type="checkbox" wire:model="request_certificate" @disabled(trim($hostname) === '' || $https_policy === 'disabled') class="rounded border-slate-300 disabled:opacity-50">
                    Request a Let’s Encrypt certificate (HTTP-01)
                </label>
            </div>
        @endif

        {{-- Step 4: Database --}}
        @if ($step === 4)
            <div class="space-y-4">
                <label class="inline-flex items-center gap-2 text-sm text-slate-700">
                    <input type="checkbox" wire:model.live="provision_database" class="rounded border-slate-300">
                    Provision a managed database for this application
                </label>

                @if ($provision_database)
                    <div>
                        <label for="engine" class="block text-sm font-medium text-slate-700">Engine</label>
                        <select id="engine" wire:model="engine" class="mt-1.5 block w-full rounded-lg border border-slate-300 bg-white px-3.5 py-2.5 text-sm text-slate-900 shadow-xs focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                            @foreach ($engines as $option)
                                <option value="{{ $option }}">{{ $option }}</option>
                            @endforeach
                        </select>
                    </div>
                    <div class="grid gap-4 sm:grid-cols-2">
                        <div>
                            <label for="database_name" class="block text-sm font-medium text-slate-700">Database name</label>
                            <input id="database_name" type="text" wire:model="database_name" placeholder="my_app_db" class="mt-1.5 block w-full rounded-lg border border-slate-300 bg-white px-3.5 py-2.5 font-mono text-sm text-slate-900 shadow-xs focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                            @error('database_name') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                        </div>
                        <div>
                            <label for="database_username" class="block text-sm font-medium text-slate-700">Username</label>
                            <input id="database_username" type="text" wire:model="database_username" placeholder="my_app_user" class="mt-1.5 block w-full rounded-lg border border-slate-300 bg-white px-3.5 py-2.5 font-mono text-sm text-slate-900 shadow-xs focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                            @error('database_username') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                        </div>
                    </div>
                    <p class="text-xs text-slate-500">Credentials are generated by the panel and stored encrypted.</p>
                @endif
            </div>
        @endif

        {{-- Step 5: Secrets --}}
        @if ($step === 5)
            <div class="space-y-4">
                <p class="text-sm text-slate-600">Optional. Encrypted environment variables for the application.</p>
                @foreach ($secrets as $index => $secret)
                    <div class="grid gap-3 sm:grid-cols-2" wire:key="secret-row-{{ $index }}">
                        <div>
                            <label for="secret_name_{{ $index }}" class="block text-sm font-medium text-slate-700">Name</label>
                            <input id="secret_name_{{ $index }}" type="text" wire:model="secrets.{{ $index }}.name" placeholder="MAIL_HOST" class="mt-1.5 block w-full rounded-lg border border-slate-300 bg-white px-3.5 py-2.5 text-sm text-slate-900 shadow-xs focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                            @error("secrets.$index.name") <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                        </div>
                        <div>
                            <label for="secret_value_{{ $index }}" class="block text-sm font-medium text-slate-700">Value</label>
                            <div class="mt-1.5 flex gap-2">
                                <input id="secret_value_{{ $index }}" type="password" wire:model="secrets.{{ $index }}.value" autocomplete="off" class="block w-full rounded-lg border border-slate-300 bg-white px-3.5 py-2.5 text-sm text-slate-900 shadow-xs focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                                <x-ui.button type="button" variant="danger" wire:click="removeSecret({{ $index }})">Remove</x-ui.button>
                            </div>
                            @error("secrets.$index.value") <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                        </div>
                    </div>
                @endforeach
                <x-ui.button type="button" variant="secondary" wire:click="addSecret">Add another</x-ui.button>
            </div>
        @endif

        {{-- Step 6: Schedule --}}
        @if ($step === 6)
            <div class="space-y-4">
                <label class="inline-flex items-center gap-2 text-sm text-slate-700">
                    <input type="checkbox" wire:model.live="schedule_enabled" class="rounded border-slate-300">
                    Add a scheduled job
                </label>

                @if ($schedule_enabled)
                    <div class="grid gap-4 sm:grid-cols-2">
                        <div>
                            <label for="schedule_name" class="block text-sm font-medium text-slate-700">Name</label>
                            <input id="schedule_name" type="text" wire:model="schedule_name" placeholder="nightly" class="mt-1.5 block w-full rounded-lg border border-slate-300 bg-white px-3.5 py-2.5 text-sm text-slate-900 shadow-xs focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                            @error('schedule_name') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                        </div>
                        <div>
                            <label for="schedule_expression" class="block text-sm font-medium text-slate-700">Cron expression</label>
                            <input id="schedule_expression" type="text" wire:model="schedule_expression" placeholder="0 2 * * *" class="mt-1.5 block w-full rounded-lg border border-slate-300 bg-white px-3.5 py-2.5 font-mono text-sm text-slate-900 shadow-xs focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                            @error('schedule_expression') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                        </div>
                        <div>
                            <label for="schedule_timezone" class="block text-sm font-medium text-slate-700">Timezone</label>
                            <input id="schedule_timezone" type="text" wire:model="schedule_timezone" class="mt-1.5 block w-full rounded-lg border border-slate-300 bg-white px-3.5 py-2.5 text-sm text-slate-900 shadow-xs focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                            @error('schedule_timezone') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                        </div>
                        <div>
                            <label for="schedule_command_reference" class="block text-sm font-medium text-slate-700">Command</label>
                            <input id="schedule_command_reference" type="text" wire:model="schedule_command_reference" placeholder="schedule:run" class="mt-1.5 block w-full rounded-lg border border-slate-300 bg-white px-3.5 py-2.5 font-mono text-sm text-slate-900 shadow-xs focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                            @error('schedule_command_reference') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                        </div>
                    </div>
                @endif
            </div>
        @endif

        {{-- Step 7: Review --}}
        @if ($step === 7)
            <div class="space-y-4">
                <dl class="grid gap-3 text-sm sm:grid-cols-2">
                    <div>
                        <dt class="text-slate-600">Application</dt>
                        <dd class="font-medium">{{ $name !== '' ? $name : '—' }} <span class="font-mono text-xs text-slate-500">{{ $slug }}</span></dd>
                    </div>
                    <div>
                        <dt class="text-slate-600">Runtime</dt>
                        <dd class="font-medium">{{ $runtime }} {{ $runtime_version }}</dd>
                    </div>
                    <div>
                        <dt class="text-slate-600">Source</dt>
                        <dd class="break-all font-medium">{{ $repository_url !== '' ? $repository_url : 'Not set' }} {{ $git_ref !== '' ? '('.$git_ref.')' : '' }}</dd>
                    </div>
                    <div>
                        <dt class="text-slate-600">Domain</dt>
                        <dd class="font-medium">{{ $hostname !== '' ? $hostname : 'Not set' }} {{ $hostname !== '' ? '('.$https_policy.')' : '' }}</dd>
                    </div>
                    <div>
                        <dt class="text-slate-600">Certificate</dt>
                        <dd class="font-medium">{{ $hostname !== '' && $request_certificate ? 'Request managed (HTTP-01)' : 'Not requested' }}</dd>
                    </div>
                    <div>
                        <dt class="text-slate-600">Database</dt>
                        <dd class="font-medium">{{ $provision_database ? $engine.' · '.$database_name : 'Not requested' }}</dd>
                    </div>
                </dl>

                <label class="inline-flex items-center gap-2 text-sm text-slate-700">
                    <input type="checkbox" wire:model="run_deploy" @disabled(trim($repository_url) === '') class="rounded border-slate-300 disabled:opacity-50">
                    Deploy after creating
                </label>
                @if (trim($repository_url) === '')
                    <p class="text-xs text-slate-500">A repository URL is required to deploy.</p>
                @else
                    <label class="inline-flex items-center gap-2 text-sm text-slate-700">
                        <input type="checkbox" wire:model="run_migrations" class="rounded border-slate-300">
                        Run database migrations on first deploy
                    </label>
                    @if ($run_migrations)
                        <label class="inline-flex items-start gap-2 text-sm text-slate-700">
                            <input type="checkbox" wire:model="migration_recovery_ack" class="mt-0.5 rounded border-slate-300">
                            I have a current recovery point for this application's database.
                        </label>
                        @error('migration_recovery_ack') <p role="alert" class="text-sm text-red-700">{{ $message }}</p> @enderror
                    @endif
                @endif
            </div>
        @endif
    </x-ui.panel>

    <div class="mt-6 flex items-center justify-between gap-3">
        <div>
            @if ($step > 0)
                <x-ui.button type="button" variant="secondary" wire:click="back">Back</x-ui.button>
            @endif
        </div>
        <div>
            @if ($step < count($steps) - 1)
                <x-ui.button type="button" wire:click="next">Next</x-ui.button>
            @else
                <x-ui.button type="button" wire:click="finish">Create application</x-ui.button>
            @endif
        </div>
    </div>
</main>
