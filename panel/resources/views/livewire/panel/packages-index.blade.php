<main>
    <header class="mb-6">
        <h1 class="text-2xl font-semibold tracking-tight text-slate-900">Packages &amp; services</h1>
    </header>

    <x-ui.capability-notice :snapshot="$snapshot" :stale-after="$staleAfter" />

    @error('agent') <p role="alert" class="mb-6 rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-800">{{ $message }}</p> @enderror

    <div class="grid gap-6 lg:grid-cols-2">
        <x-ui.panel title="Managed services">
            @if ($snapshot->services === [])
                <p class="mt-4 text-sm text-slate-600">No services reported.</p>
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

        <div class="space-y-6">
            <x-ui.panel title="Package catalog">
                <p class="mt-2 text-sm text-slate-600">Install from the built-in or operator-approved catalog.</p>
                <form wire:submit="install" class="mt-4 space-y-3">
                    @error('package') <p role="alert" class="text-sm text-red-700">{{ $message }}</p> @enderror
                    <div>
                        <label for="package" class="block text-sm font-medium text-slate-700">Package</label>
                        <select id="package" wire:model="packageId" class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                            <option value="">Select a package</option>
                            @foreach ($packages as $package)
                                <option value="{{ $package }}">{{ $package }}</option>
                            @endforeach
                            @foreach ($catalog as $package)
                                <option value="{{ $package->name }}">{{ $package->name }}</option>
                            @endforeach
                        </select>
                    </div>
                    <x-ui.button type="submit" variant="secondary">Request install</x-ui.button>
                </form>
            </x-ui.panel>

            <x-ui.panel title="Operator catalog">
                <form wire:submit="addPackage" class="mt-4 space-y-3">
                    <div>
                        <label for="packageName" class="block text-sm font-medium text-slate-700">Add a package</label>
                        <input id="packageName" type="text" wire:model="packageName" placeholder="redis-server" class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 font-mono text-sm focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                        <p class="mt-1 text-xs text-slate-500">Lowercase name; approved here and registered with the agent.</p>
                        @error('name') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                    </div>
                    <x-ui.button type="submit" variant="secondary">Approve package</x-ui.button>
                </form>

                @if ($catalog->isNotEmpty())
                    <ul class="mt-4 divide-y divide-slate-100 text-sm">
                        @foreach ($catalog as $package)
                            <li class="flex items-center justify-between gap-3 py-2" wire:key="pkg-{{ $package->id }}">
                                <span class="font-mono">{{ $package->name }}</span>
                                <x-ui.button type="button" variant="danger" wire:click="removePackage('{{ $package->id }}')">Remove</x-ui.button>
                            </li>
                        @endforeach
                    </ul>
                @endif
            </x-ui.panel>

            <x-ui.panel title="Updates">
                <p class="mt-2 text-sm text-slate-600">Apply pending upgrades. Only already-installed packages are upgraded (never new installs).</p>
                <div class="mt-4 flex flex-wrap gap-2">
                    <x-ui.button type="button" variant="secondary" wire:click="applySecurity">Apply security updates</x-ui.button>
                    <x-ui.button type="button" wire:click="applyAll">Apply all updates</x-ui.button>
                </div>
            </x-ui.panel>
        </div>
    </div>
</main>
