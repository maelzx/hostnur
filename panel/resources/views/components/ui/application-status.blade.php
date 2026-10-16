@props(['status'])

<x-ui.badge :tone="$status->tone()">{{ $status->label() }}</x-ui.badge>
