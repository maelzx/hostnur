@props(['tone' => 'neutral'])

@php
    $tones = [
        'success' => 'bg-emerald-100 text-emerald-800',
        'warning' => 'bg-amber-100 text-amber-800',
        'danger' => 'bg-red-100 text-red-800',
        'info' => 'bg-indigo-100 text-indigo-800',
        'neutral' => 'bg-slate-100 text-slate-700',
    ];
@endphp

<span {{ $attributes->merge(['class' => 'inline-flex items-center rounded-md px-2 py-0.5 text-xs font-medium '.($tones[$tone] ?? $tones['neutral'])]) }}>
    {{ $slot }}
</span>
