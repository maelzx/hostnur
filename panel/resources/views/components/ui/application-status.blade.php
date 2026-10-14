@props(['status', 'deployment' => null])

<span class="inline-flex flex-wrap items-center gap-1">
    <x-ui.badge :tone="$status->tone()">{{ $status->label() }}</x-ui.badge>
    @if ($deployment?->isSimulated())
        <x-ui.badge tone="warning">simulated</x-ui.badge>
    @endif
</span>
