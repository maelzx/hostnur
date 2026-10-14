<main>
    <header class="mb-6">
        <h1 class="text-2xl font-semibold tracking-tight">Server</h1>
    </header>

    <x-ui.capability-notice :snapshot="$snapshot" :stale-after="(int) config('hostnur.dashboard.stale_after_seconds', 120)" />

    @error('agent') <p role="alert" class="mb-6 rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-800">{{ $message }}</p> @enderror

    <div class="grid gap-6 lg:grid-cols-2">
        <x-ui.panel title="Managed services">
            @if ($snapshot->services === [])
                <p class="mt-4 text-sm text-slate-600">No services reported by the agent.</p>
            @else
                <ul class="mt-4 divide-y divide-slate-100 text-sm">
                    @foreach ($snapshot->services as $service)
                        <li class="flex items-center justify-between gap-3 py-2">
                            <span class="font-mono font-medium">{{ $service['name'] }}</span>
                            <x-ui.badge :tone="$service['status'] === 'running' ? 'success' : 'warning'">{{ $service['status'] }}</x-ui.badge>
                        </li>
                    @endforeach
                </ul>
            @endif

            <form wire:submit="control" class="mt-4 space-y-3">
                @error('service') <p role="alert" class="text-sm text-red-700">{{ $message }}</p> @enderror
                <div class="grid gap-3 sm:grid-cols-2">
                    <div>
                        <label for="service" class="block text-sm font-medium text-slate-700">Service</label>
                        <select id="service" wire:model="serviceId" class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm">
                            <option value="">Select a reported service</option>
                            @foreach ($snapshot->services as $service)
                                <option value="{{ $service['name'] }}">{{ $service['name'] }}</option>
                            @endforeach
                        </select>
                    </div>
                    <div>
                        <label for="service_action" class="block text-sm font-medium text-slate-700">Action</label>
                        <select id="service_action" wire:model="service_action" class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm">
                            <option value="start">start</option>
                            <option value="stop">stop</option>
                            <option value="restart">restart</option>
                        </select>
                    </div>
                </div>
                <x-ui.button type="submit" variant="secondary">Request service control</x-ui.button>
            </form>
        </x-ui.panel>

        <x-ui.panel title="Package catalog">
            <p class="mt-2 text-sm text-slate-600">Approved package IDs only; no arbitrary APT flags or repositories.</p>
            <form wire:submit="install" class="mt-4 space-y-3">
                @error('package') <p role="alert" class="text-sm text-red-700">{{ $message }}</p> @enderror
                <div>
                    <label for="package" class="block text-sm font-medium text-slate-700">Package</label>
                    <select id="package" wire:model="packageId" class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm">
                        <option value="">Select an approved package</option>
                        @foreach ($packages as $package)
                            <option value="{{ $package }}">{{ $package }}</option>
                        @endforeach
                    </select>
                </div>
                <x-ui.button type="submit" variant="secondary">Request install</x-ui.button>
            </form>
        </x-ui.panel>
    </div>

    <x-ui.panel title="Firewall (read-only)" class="mt-6">
        @if ($snapshot->firewall === null)
            <p class="mt-2 text-sm text-slate-600">The firewall state is not reported by this agent. No firewall changes are available.</p>
        @else
            <dl class="mt-2 grid gap-3 text-sm sm:grid-cols-3">
                <div><dt class="text-slate-600">Enabled</dt><dd class="font-medium">{{ $snapshot->firewall['enabled'] ? 'yes' : 'no' }}</dd></div>
                <div><dt class="text-slate-600">Incoming policy</dt><dd class="font-medium">{{ $snapshot->firewall['incoming_policy'] }}</dd></div>
                <div><dt class="text-slate-600">Rules</dt><dd class="font-medium">{{ $snapshot->firewall['rule_count'] }}</dd></div>
            </dl>
            <p class="mt-2 text-xs text-slate-500">Read-only summary; firewall mutations are not available.</p>
        @endif
    </x-ui.panel>
</main>
