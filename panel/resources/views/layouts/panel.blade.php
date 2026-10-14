<!DOCTYPE html>
<html lang="{{ str_replace('_', '-', app()->getLocale()) }}">
    <head>
        <meta charset="utf-8">
        <meta name="viewport" content="width=device-width, initial-scale=1.0">

        <title>{{ $title ?? config('app.name') }}</title>

        @vite(['resources/css/app.css', 'resources/js/app.js'])

        @livewireStyles
    </head>
    <body class="min-h-dvh bg-slate-100 font-sans text-slate-900 antialiased">
        <a
            href="#main-content"
            class="sr-only focus:not-sr-only focus:absolute focus:left-4 focus:top-4 focus:z-50 focus:rounded-lg focus:bg-indigo-600 focus:px-4 focus:py-2 focus:text-sm focus:font-semibold focus:text-white"
        >
            Skip to content
        </a>

        <div class="lg:flex">
            <x-panel.nav />

            <main id="main-content" class="min-w-0 flex-1">
                <div class="mx-auto w-full max-w-6xl px-4 py-8 sm:px-8">
                    @if (session('status'))
                        <p role="status" class="mb-6 rounded-lg border border-emerald-200 bg-emerald-50 px-4 py-3 text-sm text-emerald-800">
                            {{ session('status') }}
                        </p>
                    @endif

                    {{ $slot }}
                </div>
            </main>
        </div>

        @livewireScripts
    </body>
</html>
