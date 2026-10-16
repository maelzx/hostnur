<main>
    <header class="mb-6">
        <h1 class="text-2xl font-semibold tracking-tight text-slate-900">Firewall</h1>
    </header>

    <x-ui.capability-notice :snapshot="$snapshot" :stale-after="$staleAfter" />

    <x-ui.panel title="Current state">
        @if ($snapshot->firewall === null)
            <p class="mt-2 text-sm text-slate-600">Not reported by this agent.</p>
        @else
            <dl class="mt-2 grid gap-3 text-sm sm:grid-cols-3">
                <div><dt class="text-slate-600">Enabled</dt><dd class="font-medium">{{ $snapshot->firewall['enabled'] ? 'yes' : 'no' }}</dd></div>
                <div><dt class="text-slate-600">Incoming policy</dt><dd class="font-medium">{{ $snapshot->firewall['incoming_policy'] }}</dd></div>
                <div><dt class="text-slate-600">Rules</dt><dd class="font-medium">{{ $snapshot->firewall['rule_count'] }}</dd></div>
            </dl>
        @endif
    </x-ui.panel>

    <x-ui.panel title="Rules" class="mt-6">
        <p class="mt-2 text-sm text-slate-600">
            Port 80 and 443 stay open for web traffic. Add, edit and remove other rules
            once the agent supports firewall changes — rule management is planned and
            not available yet.
        </p>
    </x-ui.panel>
</main>
