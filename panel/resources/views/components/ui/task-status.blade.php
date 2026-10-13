@props(['status'])

@php
    $tone = match ($status) {
        \App\Enums\TaskStatus::Succeeded => 'success',
        \App\Enums\TaskStatus::Failed => 'danger',
        \App\Enums\TaskStatus::Cancelled, \App\Enums\TaskStatus::Unknown => 'warning',
        \App\Enums\TaskStatus::Running, \App\Enums\TaskStatus::Queued => 'info',
        default => 'neutral',
    };
@endphp

<x-ui.badge :tone="$tone">{{ $status->label() }}</x-ui.badge>
