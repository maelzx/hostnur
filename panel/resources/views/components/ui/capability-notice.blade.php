@props(['snapshot', 'staleAfter' => 120])

@if ($snapshot->mode === \App\Enums\AgentMode::Fake)
    <p role="status" class="mb-6 rounded-xl border border-amber-300 bg-amber-50 px-4 py-3 text-sm text-amber-900">
        <strong>Simulated agent mode.</strong> Values are samples, not read from this server.
    </p>
@elseif (! $snapshot->available)
    <p role="status" class="mb-6 rounded-xl border border-slate-300 bg-slate-100 px-4 py-3 text-sm text-slate-700">
        The agent is unavailable. {{ $snapshot->reason }}
    </p>
@elseif ($snapshot->isStale($staleAfter))
    <p role="status" class="mb-6 rounded-xl border border-amber-300 bg-amber-50 px-4 py-3 text-sm text-amber-900">
        The last observation is stale ({{ $snapshot->sampledAt }}).
    </p>
@endif
