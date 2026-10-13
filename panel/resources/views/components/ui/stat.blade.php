@props(['label', 'value', 'unit' => null, 'hint' => null, 'tone' => 'neutral'])

<div {{ $attributes->merge(['class' => 'rounded-2xl border border-slate-200 bg-white p-4']) }}>
    <p class="text-xs font-semibold uppercase tracking-wide text-slate-500">{{ $label }}</p>
    <p class="mt-1 text-2xl font-semibold {{ $tone === 'warning' ? 'text-amber-800' : 'text-slate-900' }}">
        {{ $value }}@if ($unit)<span class="ml-1 text-sm font-normal text-slate-500">{{ $unit }}</span>@endif
    </p>
    @if ($hint)
        <p class="mt-1 text-xs text-slate-500">{{ $hint }}</p>
    @endif
</div>
