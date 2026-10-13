@props(['title' => null])

<section {{ $attributes->merge(['class' => 'rounded-2xl border border-slate-200 bg-white p-6']) }}>
    @if ($title)
        <h2 class="text-lg font-semibold">{{ $title }}</h2>
    @endif

    {{ $slot }}
</section>
