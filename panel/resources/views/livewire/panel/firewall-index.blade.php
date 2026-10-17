@php
    $rules = $snapshot->firewall['rules'] ?? [];
    $protected = fn (int $port): bool => in_array($port, [80, 443], true);
@endphp

<main>
    <header class="mb-6">
        <h1 class="text-2xl font-semibold tracking-tight text-slate-900">Firewall</h1>
    </header>

    <x-ui.capability-notice :snapshot="$snapshot" :stale-after="$staleAfter" />

    @error('agent') <p role="alert" class="mb-6 rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-800">{{ $message }}</p> @enderror

    <div class="grid gap-6 lg:grid-cols-2">
        <x-ui.panel title="Rules">
            @if ($snapshot->firewall === null)
                <p class="mt-2 text-sm text-slate-600">Not reported by this agent.</p>
            @elseif ($snapshot->firewall['rules'] === [])
                <p class="mt-2 text-sm text-slate-600">No rules reported.</p>
            @else
                <ul class="mt-4 divide-y divide-slate-100 text-sm">
                    @foreach ($rules as $rule)
                        <li class="flex items-center justify-between gap-3 py-2" wire:key="rule-{{ $rule['port'] }}-{{ $rule['protocol'] }}-{{ $rule['source'] }}">
                            <span class="font-mono">{{ $rule['port'] }}/{{ $rule['protocol'] }}</span>
                            <span class="flex-1 text-slate-500">{{ $rule['source'] }}</span>
                            <x-ui.badge :tone="$rule['action'] === 'allow' ? 'success' : 'danger'">{{ $rule['action'] }}</x-ui.badge>
                            @if ($protected($rule['port']))
                                <span class="text-xs text-slate-400">managed</span>
                            @else
                                <x-ui.button
                                    type="button"
                                    variant="danger"
                                    wire:click="removeRule({{ (int) $rule['port'] }}, @js($rule['protocol']), @js($rule['source']), @js($rule['action']))"
                                >Remove</x-ui.button>
                            @endif
                        </li>
                    @endforeach
                </ul>
            @endif
        </x-ui.panel>

        <div class="space-y-6">
            <x-ui.panel title="Add rule">
                <form wire:submit="addRule" class="mt-4 space-y-3">
                    <div class="grid gap-3 sm:grid-cols-2">
                        <div>
                            <label for="port" class="block text-sm font-medium text-slate-700">Port</label>
                            <input id="port" type="number" min="1" max="65535" wire:model="port" class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                            @error('port') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                        </div>
                        <div>
                            <label for="protocol" class="block text-sm font-medium text-slate-700">Protocol</label>
                            <select id="protocol" wire:model="protocol" class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                                <option value="tcp">tcp</option>
                                <option value="udp">udp</option>
                            </select>
                        </div>
                        <div>
                            <label for="source" class="block text-sm font-medium text-slate-700">Source</label>
                            <input id="source" type="text" wire:model="source" placeholder="any" class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                            <p class="mt-1 text-xs text-slate-500">"any" or an IP/CIDR.</p>
                            @error('source') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                        </div>
                        <div>
                            <label for="action" class="block text-sm font-medium text-slate-700">Action</label>
                            <select id="action" wire:model="action" class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30">
                                <option value="allow">allow</option>
                                <option value="deny">deny</option>
                            </select>
                        </div>
                    </div>
                    <x-ui.button type="submit">Add rule</x-ui.button>
                </form>
            </x-ui.panel>

            <x-ui.panel title="Web access">
                <p class="mt-2 text-sm text-slate-600">Ports 80 and 443 are always kept open. Enable HTTPS-only to close port 80 once 443 is live.</p>
                <form wire:submit="setWeb" class="mt-4 space-y-3">
                    <label class="inline-flex items-center gap-2 text-sm text-slate-700">
                        <input type="checkbox" wire:model="https_only" class="rounded border-slate-300">
                        HTTPS only (close port 80)
                    </label>
                    <div>
                        <x-ui.button type="submit" variant="secondary">Apply web policy</x-ui.button>
                    </div>
                </form>
            </x-ui.panel>
        </div>
    </div>
</main>
