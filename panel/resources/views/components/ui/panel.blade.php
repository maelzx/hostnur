@props(['title' => null])

<section {{ $attributes->merge(['class' => 'rounded-xl border border-slate-200 bg-white p-6 shadow-xs']) }}>
    @if ($title)
        <h2 class="text-lg font-semibold tracking-tight text-slate-900">{{ $title }}</h2>
    @endif

    {{ $slot }}
</section>
