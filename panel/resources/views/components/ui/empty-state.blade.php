@props(['title' => 'Nothing here yet'])

<section {{ $attributes->merge(['class' => 'rounded-xl border border-dashed border-slate-300 bg-white p-8 text-center text-sm text-slate-600']) }}>
    <p class="font-medium text-slate-800">{{ $title }}</p>

    @if (trim((string) $slot) !== '')
        <div class="mt-1">{{ $slot }}</div>
    @endif
</section>
