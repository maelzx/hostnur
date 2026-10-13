@props(['variant' => 'primary', 'type' => 'button'])

@php
    $variants = [
        'primary' => 'bg-indigo-700 text-white hover:bg-indigo-800',
        'secondary' => 'border border-slate-300 text-slate-700 hover:bg-slate-100',
        'danger' => 'border border-red-300 text-red-700 hover:bg-red-50',
    ];
@endphp

<button
    type="{{ $type }}"
    {{ $attributes->merge(['class' => 'rounded-lg px-4 py-2 text-sm font-semibold focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-2 '.($variants[$variant] ?? $variants['primary'])]) }}
>
    {{ $slot }}
</button>
