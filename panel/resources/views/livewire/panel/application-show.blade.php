<main>
    <header class="mb-6">
        <p class="text-sm text-slate-500">
            <a href="{{ route('panel.applications.index') }}" class="font-medium text-indigo-700 hover:underline">Applications</a>
        </p>
        <h1 class="mt-2 break-words text-2xl font-semibold tracking-tight">{{ $application->name }}</h1>
        <p class="mt-2 flex flex-wrap items-center gap-2 text-sm text-slate-600">
            <span class="font-mono">{{ $application->slug }}</span>
            <x-ui.application-status :status="$application->status()" :deployment="$application->latestDeployment" />
            <span>provider {{ $application->provider }}</span>
            <span>revision {{ $application->current_revision }}</span>
        </p>
    </header>

    @if (session('status'))
        <div class="mb-6 rounded-lg border border-green-200 bg-green-50 px-4 py-3 text-sm text-green-800">{{ session('status') }}</div>
    @endif

    {{-- Progress checklist --}}
    <x-ui.panel title="Setup checklist" class="mb-6">
        <p class="mt-2 text-sm text-slate-600">
            Only <strong>Source</strong> and <strong>Runtime</strong> are required; the rest is optional.
        </p>
        <ol class="mt-4 grid gap-2 text-sm sm:grid-cols-2">
            @php
                $checks = [
                    ['Repository saved', trim($repository_url) !== ''],
                    ['Runtime configured', trim($runtime) !== ''],
                    ['Workers / scheduler set', (int) $workers > 0 || $scheduler],
                    ['Secrets added (optional)', $secrets->isNotEmpty()],
                    ['Domain added (optional)', $domains->isNotEmpty()],
                ];
            @endphp
            @foreach ($checks as [$label, $done])
                <li class="flex items-center gap-2">
                    <span class="inline-flex h-5 w-5 items-center justify-center rounded-full text-xs font-bold {{ $done ? 'bg-green-100 text-green-700' : 'bg-slate-100 text-slate-400' }}">{{ $done ? '✓' : '•' }}</span>
                    <span class="{{ $done ? 'text-slate-700' : 'text-slate-500' }}">{{ $label }}</span>
                </li>
            @endforeach
        </ol>
        <div class="mt-4 flex flex-wrap gap-3">
            <a href="{{ route('panel.applications.operations', ['application' => $application->id]) }}" class="inline-flex items-center justify-center gap-2 rounded-lg bg-indigo-600 px-4 py-2 text-sm font-semibold text-white shadow-xs transition-colors hover:bg-indigo-700">Deploy &amp; operations</a>
            <a href="{{ route('panel.deployments.index') }}" class="inline-flex rounded-lg border border-slate-300 px-4 py-2 text-sm font-semibold text-slate-700 hover:bg-slate-50">Deployment history</a>
        </div>
    </x-ui.panel>

    <div class="space-y-6">
        {{-- Step 1: Source --}}
        <x-ui.panel>
            <div class="flex items-start gap-3">
                <span class="mt-0.5 inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-indigo-600 text-sm font-bold text-white">1</span>
                <div>
                    <h2 class="text-lg font-semibold text-slate-900">Connect your repository</h2>
                </div>
            </div>
            <form wire:submit="saveSource" class="mt-4 space-y-3">
                <div>
                    <label for="repository_url" class="block text-sm font-medium text-slate-700">Repository URL <span class="text-red-600">*</span></label>
                    <input id="repository_url" type="text" wire:model="repository_url" placeholder="https://github.com/org/repo.git" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm">
                    <p class="mt-1 text-xs text-slate-500">Use the HTTPS clone URL, ending in <code>.git</code>.</p>
                    @error('repository_url') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                </div>
                <div class="grid gap-3 sm:grid-cols-2">
                    <div>
                        <label for="git_ref" class="block text-sm font-medium text-slate-700">Branch, tag, or commit</label>
                        <input id="git_ref" type="text" wire:model="git_ref" placeholder="main" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm">
                        <p class="mt-1 text-xs text-slate-500">Leave blank to use <code>main</code>. A commit SHA pins the exact code.</p>
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
                        <p class="mt-1 text-xs text-slate-500">For private repositories only.</p>
                        @error('source_credential_id') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                    </div>
                </div>
                <x-ui.button type="submit">Save source</x-ui.button>
            </form>

            <details class="mt-4 rounded-lg border border-slate-200 p-3">
                <summary class="cursor-pointer text-sm font-medium text-slate-700">Private repository? Add a credential (optional)</summary>
                <form wire:submit="storeCredential" class="mt-3 space-y-3">
                    <div class="grid gap-3 sm:grid-cols-2">
                        <div>
                            <label for="credential_label" class="block text-sm font-medium text-slate-700">Label</label>
                            <input id="credential_label" type="text" wire:model="credential_label" placeholder="GitHub read-only token" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm">
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
            </details>
        </x-ui.panel>

        {{-- Step 2: Runtime, build & scale --}}
        <x-ui.panel>
            <div class="flex items-start gap-3">
                <span class="mt-0.5 inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-indigo-600 text-sm font-bold text-white">2</span>
                <div class="flex-1">
                    <h2 class="text-lg font-semibold text-slate-900">Runtime, build &amp; scale</h2>
                    <p class="mt-1 text-sm text-slate-600">
                        For a Laravel app choose <strong>php</strong>, then use the recommended settings.
                    </p>
                </div>
            </div>

            <form wire:submit="saveConfiguration" class="mt-4 space-y-4">
                <div class="grid gap-4 sm:grid-cols-2">
                    <div>
                        <label for="runtime" class="block text-sm font-medium text-slate-700">Runtime <span class="text-red-600">*</span></label>
                        <select id="runtime" wire:model="runtime" class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm">
                            @foreach ($runtimes as $option)<option value="{{ $option }}">{{ $option }}</option>@endforeach
                        </select>
                        @error('runtime') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                    </div>
                    <div>
                        <label for="runtime_version" class="block text-sm font-medium text-slate-700">PHP version</label>
                        <input id="runtime_version" type="text" wire:model="runtime_version" placeholder="8.3" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm">
                        <p class="mt-1 text-xs text-slate-500">Server-supported version, e.g. 8.3 or 8.4.</p>
                        @error('runtime_version') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                    </div>
                    <div>
                        <label for="workers" class="block text-sm font-medium text-slate-700">Queue workers</label>
                        <input id="workers" type="number" min="0" max="10" wire:model="workers" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm">
                        <p class="mt-1 text-xs text-slate-500">Set to <code>1</code> if the app queues email.</p>
                        @error('workers') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                    </div>
                    <div>
                        <label for="health_path" class="block text-sm font-medium text-slate-700">Health check path</label>
                        <input id="health_path" type="text" wire:model="health_path" placeholder="up" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm">
                        <p class="mt-1 text-xs text-slate-500">Must return 200. Laravel default: <code>/up</code>.</p>
                        @error('health_path') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                    </div>
                </div>

                <label class="flex items-center gap-2 text-sm text-slate-700">
                    <input type="checkbox" wire:model="scheduler" class="rounded border-slate-300">
                    Run the Laravel scheduler (<code>schedule:run</code> every minute)
                </label>

                <details class="rounded-lg border border-slate-200 p-3">
                    <summary class="cursor-pointer text-sm font-medium text-slate-700">Advanced (build/start commands, persistent storage)</summary>
                    <div class="mt-3 grid gap-4 sm:grid-cols-2">
                        <div>
                            <label for="build_command" class="block text-sm font-medium text-slate-700">Extra build command</label>
                            <input id="build_command" type="text" wire:model="build_command" placeholder="(handled automatically)" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm">
                            <p class="mt-1 text-xs text-slate-500">Runs automatically when detected.</p>
                            @error('build_command') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                        </div>
                        <div>
                            <label for="start_command" class="block text-sm font-medium text-slate-700">Start command</label>
                            <input id="start_command" type="text" wire:model="start_command" placeholder="(blank — served by PHP-FPM)" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm">
                            @error('start_command') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                        </div>
                        <div class="sm:col-span-2">
                            <label for="persistent_paths_text" class="block text-sm font-medium text-slate-700">Persistent paths (one per line)</label>
                            <textarea id="persistent_paths_text" wire:model="persistent_paths_text" rows="3" placeholder="storage/app" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm"></textarea>
                            <p class="mt-1 text-xs text-slate-500">Kept across deploys, e.g. <code>storage/app</code>.</p>
                            @error('persistent_paths') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                            @error('persistent_paths.*') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                        </div>
                    </div>
                </details>

                <div class="flex flex-wrap items-center gap-3">
                    <x-ui.button type="button" variant="secondary" wire:click="applyLaravelDefaults">Use recommended Laravel settings</x-ui.button>
                    <x-ui.button type="submit">Save as new revision</x-ui.button>
                </div>
            </form>

            <h3 class="mt-6 text-sm font-semibold uppercase tracking-wide text-slate-500">Recent revisions</h3>
            <ul class="mt-2 text-sm text-slate-600">
                @forelse ($revisions as $revision)
                    <li class="py-1" wire:key="rev-{{ $revision->id }}">Revision {{ $revision->revision }} · {{ $revision->created_at?->toDayDateTimeString() }}</li>
                @empty
                    <li class="py-1 text-slate-500">No revisions yet.</li>
                @endforelse
            </ul>
        </x-ui.panel>

        {{-- Step 3: Secrets --}}
        <x-ui.panel>
            <div class="flex items-start gap-3">
                <span class="mt-0.5 inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-indigo-600 text-sm font-bold text-white">3</span>
                <div>
                    <h2 class="text-lg font-semibold text-slate-900">Environment secrets <span class="text-sm font-normal text-slate-500">(optional)</span></h2>
                    <p class="mt-1 text-sm text-slate-600">
                        Encrypted and never shown again. <code>.env.example</code> values apply automatically.
                    </p>
                </div>
            </div>
            <form wire:submit="setSecret" class="mt-4 grid gap-3 sm:grid-cols-2">
                <div>
                    <label for="secret_name" class="block text-sm font-medium text-slate-700">Variable name</label>
                    <input id="secret_name" type="text" wire:model="secret_name" placeholder="MAIL_HOST" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm">
                    @error('secret_name') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                </div>
                <div>
                    <label for="secret_value" class="block text-sm font-medium text-slate-700">Value (write-only)</label>
                    <input id="secret_value" type="password" wire:model="secret_value" autocomplete="off" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm">
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

        {{-- Step 4: Domains --}}
        <x-ui.panel>
            <div class="flex items-start gap-3">
                <span class="mt-0.5 inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-indigo-600 text-sm font-bold text-white">4</span>
                <div>
                    <h2 class="text-lg font-semibold text-slate-900">Domain &amp; HTTPS <span class="text-sm font-normal text-slate-500">(optional)</span></h2>
                    <p class="mt-1 text-sm text-slate-600">
                        Point the domain’s A record here, then enable HTTPS.
                    </p>
                </div>
            </div>
            <form wire:submit="addDomain" class="mt-4 grid gap-3 sm:grid-cols-2">
                <div>
                    <label for="hostname" class="block text-sm font-medium text-slate-700">Hostname</label>
                    <input id="hostname" type="text" wire:model="hostname" placeholder="app.example.com" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm">
                    @error('hostname') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                </div>
                <div>
                    <label for="https_policy" class="block text-sm font-medium text-slate-700">HTTPS</label>
                    <select id="https_policy" wire:model="https_policy" class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm">
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
                        <li class="flex items-center justify-between gap-3 py-2" wire:key="domain-{{ $domain->id }}">
                            <span class="font-mono font-medium">{{ $domain->hostname }}</span>
                            <span class="text-xs text-slate-500">{{ $domain->https_policy->value }}</span>
                            <x-ui.button type="button" variant="danger" wire:click="removeDomain('{{ $domain->id }}')">Remove</x-ui.button>
                        </li>
                    @endforeach
                </ul>
            @endif
        </x-ui.panel>

        {{-- Step 5: Deploy --}}
        <x-ui.panel>
            <div class="flex items-start gap-3">
                <span class="mt-0.5 inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-indigo-600 text-sm font-bold text-white">5</span>
                <div>
                    <h2 class="text-lg font-semibold text-slate-900">Deploy</h2>
                    <p class="mt-1 text-sm text-slate-600">
                        Save your changes, then deploy from <strong>Deploy &amp; operations</strong>.
                    </p>
                    <div class="mt-3 flex flex-wrap gap-3">
                        <a href="{{ route('panel.applications.operations', ['application' => $application->id]) }}" class="inline-flex items-center justify-center gap-2 rounded-lg bg-indigo-600 px-4 py-2 text-sm font-semibold text-white shadow-xs transition-colors hover:bg-indigo-700">Deploy &amp; operations</a>
                        <a href="{{ route('panel.deployments.index') }}" class="inline-flex rounded-lg border border-slate-300 px-4 py-2 text-sm font-semibold text-slate-700 hover:bg-slate-50">Deployment history</a>
                    </div>
                </div>
            </div>
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
