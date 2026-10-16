@props(['label', 'value' => null, 'max' => 100, 'unit' => '%', 'hint' => null, 'tone' => null])

@php
    $percent = ($value !== null && (float) $max > 0)
        ? max(0.0, min(100.0, (float) $value / (float) $max * 100))
        : null;

    $tone ??= match (true) {
        $percent === null => 'muted',
        $percent >= 85 => 'danger',
        $percent >= 70 => 'warning',
        default => 'accent',
    };

    $stroke = match ($tone) {
        'danger' => 'stroke-red-500',
        'warning' => 'stroke-amber-500',
        'accent' => 'stroke-indigo-500',
        default => 'stroke-slate-200',
    };

    $radius = 42.0;
    $circumference = 2 * M_PI * $radius;
    $offset = $circumference * (1 - ($percent ?? 0) / 100);
    $display = $percent === null ? '—' : rtrim(rtrim(number_format($percent, 1), '0'), '.');
@endphp

<div {{ $attributes->merge(['class' => 'rounded-xl border border-slate-200 bg-white p-4 shadow-xs']) }}>
    <p class="text-xs font-semibold uppercase tracking-wide text-slate-500">{{ $label }}</p>

    <div class="mt-2 flex items-center justify-center">
        <div class="relative h-28 w-28">
            <svg viewBox="0 0 100 100" class="h-28 w-28 -rotate-90" aria-hidden="true">
                <circle cx="50" cy="50" r="{{ $radius }}" fill="none" stroke-width="10" class="stroke-slate-100" />
                @if ($percent !== null)
                    <circle
                        cx="50"
                        cy="50"
                        r="{{ $radius }}"
                        fill="none"
                        stroke-width="10"
                        stroke-linecap="round"
                        class="{{ $stroke }} transition-[stroke-dashoffset] duration-500"
                        stroke-dasharray="{{ $circumference }}"
                        stroke-dashoffset="{{ $offset }}"
                    />
                @endif
            </svg>
            <div class="absolute inset-0 flex items-center justify-center">
                <span class="text-xl font-semibold tracking-tight text-slate-900">{{ $display }}@if ($percent !== null)<span class="text-sm font-normal text-slate-500">{{ $unit }}</span>@endif</span>
            </div>
        </div>
    </div>

    @if ($hint)
        <p class="mt-2 text-center text-xs text-slate-500">{{ $hint }}</p>
    @endif
</div>
