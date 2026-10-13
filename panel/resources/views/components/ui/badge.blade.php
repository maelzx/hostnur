@props(['tone' => 'neutral'])

@php
    $tones = [
        'success' => 'bg-emerald-100 text-emerald-900',
        'warning' => 'bg-amber-100 text-amber-900',
        'danger' => 'bg-red-100 text-red-900',
        'info' => 'bg-sky-100 text-sky-900',
        'neutral' => 'bg-slate-100 text-slate-700',
    ];
@endphp

<span {{ $attributes->merge(['class' => 'inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium '.($tones[$tone] ?? $tones['neutral'])]) }}>
    {{ $slot }}
</span>
