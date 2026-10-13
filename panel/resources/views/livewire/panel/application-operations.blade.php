<main>
    <header class="mb-6">
        <p class="text-sm text-slate-500">
            <a href="{{ route('panel.applications.show', ['application' => $application->id]) }}" class="font-medium text-indigo-700 hover:underline">{{ $application->name }}</a>
        </p>
        <h1 class="mt-2 text-2xl font-semibold tracking-tight">Deploy &amp; operations</h1>
        <p class="mt-2 text-sm leading-6 text-slate-600">
            Requests are simulated while the agent is unavailable or in fake mode.
            The panel never runs Composer, Git or Artisan against hosted source.
        </p>
    </header>

    <x-ui.panel title="Preflight (panel configuration only)" class="mb-6">
        <ul class="mt-4 space-y-2 text-sm">
            @foreach ($checks as $check)
                <li class="flex items-start justify-between gap-4" wire:key="preflight-{{ $check['key'] }}">
                    <span>
                        {{ $check['label'] }}
                        @if ($check['message']) <span class="block text-xs text-slate-500">{{ $check['message'] }}</span> @endif
                    </span>
                    @if ($check['passed'])
                        <x-ui.badge tone="success">ok</x-ui.badge>
                    @elseif ($check['required'])
                        <x-ui.badge tone="danger">blocked</x-ui.badge>
                    @else
                        <x-ui.badge tone="warning">note</x-ui.badge>
                    @endif
                </li>
            @endforeach
        </ul>
        <p class="mt-4 text-xs text-slate-500">
            Preflight checks panel configuration only; repository reachability, package
            availability and host changes remain agent-dependent.
        </p>
    </x-ui.panel>

    <x-ui.panel title="Deploy" class="mb-6">
        <dl class="mt-4 grid gap-3 text-sm sm:grid-cols-2">
            <div><dt class="text-slate-600">Revision</dt><dd class="font-medium">{{ $revision?->revision ?? '—' }}</dd></div>
            <div><dt class="text-slate-600">Git ref</dt><dd class="break-all font-medium">{{ $application->git_ref ?? '—' }}</dd></div>
            <div><dt class="text-slate-600">Runtime</dt><dd class="font-medium">{{ $application->runtime }} {{ $application->runtime_version }}</dd></div>
            <div><dt class="text-slate-600">Workers / scheduler</dt><dd class="font-medium">{{ $revision?->settings['workers'] ?? 0 }} / {{ ($revision?->settings['scheduler'] ?? false) ? 'on' : 'off' }}</dd></div>
            <div class="sm:col-span-2"><dt class="text-slate-600">Mode</dt><dd class="font-medium">{{ config('hostnur.agent_mode') }}</dd></div>
        </dl>

        @error('confirm') <p role="alert" class="mt-3 rounded-lg border border-red-200 bg-red-50 px-4 py-2 text-sm text-red-800">{{ $message }}</p> @enderror

        <div class="mt-4 flex flex-wrap items-center gap-4">
            <label class="inline-flex items-center gap-2 text-sm text-slate-700">
                <input type="checkbox" wire:model.live="run_migrations" class="rounded border-slate-300">
                Run database migrations
            </label>
            <span class="text-xs text-slate-500">Code rollback is not database rollback; shared uploads are not reverted.</span>
        </div>

        <div class="mt-4 flex flex-wrap gap-3">
            <x-ui.button type="button" variant="secondary" wire:click="confirmPlan">Confirm plan</x-ui.button>
            <x-ui.button type="button" wire:click="deploy">Request deployment</x-ui.button>
        </div>
        @if ($confirmed)
            <p role="status" class="mt-2 text-sm text-emerald-800">Plan confirmed ({{ substr($planFingerprint, 0, 12) }}).</p>
        @endif
    </x-ui.panel>

    <div class="grid gap-6 lg:grid-cols-2">
        <x-ui.panel title="Restart">
            @error('restart') <p role="alert" class="mt-2 text-sm text-red-700">{{ $message }}</p> @enderror
            <p class="mt-2 text-sm text-slate-600">Restart the managed application processes.</p>
            <div class="mt-4">
                <x-ui.confirm title="Restart this application?" confirm-label="Restart" method="restart" variant="secondary">
                    <x-slot:trigger>Restart application</x-slot:trigger>
                    The request is simulated in fake mode and recorded as a task.
                </x-ui.confirm>
            </div>
        </x-ui.panel>

        <x-ui.panel title="Rollback">
            @error('release') <p role="alert" class="mt-2 text-sm text-red-700">{{ $message }}</p> @enderror
            @if ($releases->isEmpty())
                <p class="mt-2 text-sm text-slate-600">No successful releases are available to roll back to.</p>
            @else
                <div class="mt-2">
                    <label for="release" class="block text-sm font-medium text-slate-700">Known release</label>
                    <select id="release" wire:model="releaseId" class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm">
                        <option value="">Select a release</option>
                        @foreach ($releases as $release)
                            <option value="{{ $release->id }}">{{ $release->release_reference }} ({{ $release->finished_at?->toDayDateTimeString() }})</option>
                        @endforeach
                    </select>
                </div>
                <div class="mt-4">
                    <x-ui.confirm title="Roll back to this release?" confirm-label="Roll back" method="rollback" variant="danger">
                        <x-slot:trigger>Request rollback</x-slot:trigger>
                        Only a known compatible application release is permitted. Database changes are not reversed.
                    </x-ui.confirm>
                </div>
            @endif
        </x-ui.panel>
    </div>
</main>
