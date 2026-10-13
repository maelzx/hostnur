@props(['title' => 'Nothing here yet'])

<section {{ $attributes->merge(['class' => 'rounded-2xl border border-slate-200 bg-white p-6 text-sm text-slate-600']) }}>
    <p class="font-medium text-slate-800">{{ $title }}</p>

    @if (trim((string) $slot) !== '')
        <div class="mt-1">{{ $slot }}</div>
    @endif
</section>
