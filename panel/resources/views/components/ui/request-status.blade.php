@props(['state', 'mode' => null])

@php
    $tone = match ($state->value) {
        'succeeded' => 'success',
        'failed' => 'danger',
        'cancelled', 'unknown' => 'warning',
        'running' => 'info',
        default => 'neutral',
    };
@endphp

<span class="inline-flex flex-wrap items-center gap-1">
    <x-ui.badge :tone="$tone">{{ $state->label() }}</x-ui.badge>
    @if ($mode === 'fake')
        <x-ui.badge tone="warning">simulated</x-ui.badge>
    @elseif ($mode !== null)
        <x-ui.badge tone="info">{{ $mode }}</x-ui.badge>
    @endif
</span>
