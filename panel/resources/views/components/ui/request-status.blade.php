@props(['state'])

@php
    $tone = match ($state->value) {
        'succeeded' => 'success',
        'failed' => 'danger',
        'cancelled', 'unknown' => 'warning',
        'running' => 'info',
        default => 'neutral',
    };
@endphp

<x-ui.badge :tone="$tone">{{ $state->label() }}</x-ui.badge>
