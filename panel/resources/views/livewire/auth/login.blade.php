<div class="grid min-h-dvh lg:grid-cols-2">
    {{-- Brand panel --}}
    <aside class="relative hidden overflow-hidden bg-slate-900 p-12 lg:flex lg:flex-col">
        <div class="pointer-events-none absolute -right-24 -top-24 h-72 w-72 rounded-full bg-indigo-500/10 blur-2xl" aria-hidden="true"></div>

        <div class="relative flex items-center gap-3">
            <span class="inline-flex h-10 w-10 items-center justify-center rounded-xl bg-indigo-600 text-lg font-bold text-white" aria-hidden="true">H</span>
            <span class="text-lg font-semibold tracking-tight text-white">{{ config('app.name') }}</span>
        </div>

        <h2 class="relative mt-16 max-w-sm text-3xl font-semibold leading-tight tracking-tight text-white">
            Your own control panel for the VPS.
        </h2>
    </aside>

    {{-- Sign-in form --}}
    <main class="flex items-center justify-center px-6 py-12">
        <div class="w-full max-w-sm">
            <div class="mb-8 flex items-center gap-3 lg:hidden">
                <span class="inline-flex h-10 w-10 items-center justify-center rounded-xl bg-indigo-600 text-lg font-bold text-white" aria-hidden="true">H</span>
                <span class="text-lg font-semibold tracking-tight text-slate-900">{{ config('app.name') }}</span>
            </div>

            <h1 class="mb-8 text-2xl font-semibold tracking-tight text-slate-900">Sign in</h1>

            <form method="POST" action="{{ route('login.store') }}" class="space-y-5">
                @csrf

                @if (session('status'))
                    <p role="status" aria-live="polite" class="rounded-lg border border-emerald-200 bg-emerald-50 px-4 py-3 text-sm text-emerald-800">
                        {{ session('status') }}
                    </p>
                @endif

                @if ($errors->any())
                    <p role="alert" class="flex items-start gap-2 rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-800">
                        <svg class="mt-0.5 h-4 w-4 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="12" cy="12" r="10"/><path d="M12 8v4"/><path d="M12 16h.01"/></svg>
                        <span>{{ $errors->first() }}</span>
                    </p>
                @endif

                <div>
                    <label for="email" class="block text-sm font-medium text-slate-700">Email</label>
                    <input
                        id="email"
                        name="email"
                        type="email"
                        value="{{ old('email') }}"
                        autocomplete="email"
                        inputmode="email"
                        required
                        autofocus
                        class="mt-1.5 block w-full rounded-lg border border-slate-300 bg-white px-3.5 py-2.5 text-sm text-slate-900 shadow-xs transition-colors placeholder:text-slate-400 focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30"
                    >
                </div>

                <div x-data="{ show: false }">
                    <label for="password" class="block text-sm font-medium text-slate-700">Password</label>
                    <div class="relative mt-1.5">
                        <input
                            id="password"
                            name="password"
                            x-bind:type="show ? 'text' : 'password'"
                            autocomplete="current-password"
                            required
                            class="block w-full rounded-lg border border-slate-300 bg-white px-3.5 py-2.5 pr-12 text-sm text-slate-900 shadow-xs transition-colors focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30"
                        >
                        <button
                            type="button"
                            x-on:click="show = !show"
                            x-bind:aria-label="show ? 'Hide password' : 'Show password'"
                            x-bind:aria-pressed="show"
                            class="absolute inset-y-0 right-0 flex w-11 items-center justify-center rounded-r-lg text-slate-400 hover:text-slate-600 focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500/40"
                        >
                            <svg x-show="! show" class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7-10-7-10-7Z"/><circle cx="12" cy="12" r="3"/></svg>
                            <svg x-show="show" x-cloak class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m3 3 18 18"/><path d="M10.6 10.6a3 3 0 0 0 4.2 4.2"/><path d="M9.9 4.24A9.8 9.8 0 0 1 12 4c6.5 0 10 7 10 7a17.8 17.8 0 0 1-3.2 4.1"/><path d="M6.6 6.6A17.6 17.6 0 0 0 2 11s3.5 7 10 7a9.7 9.7 0 0 0 4.1-.9"/></svg>
                        </button>
                    </div>
                </div>

                <button
                    type="submit"
                    class="inline-flex w-full items-center justify-center gap-2 rounded-lg bg-indigo-600 px-4 py-2.5 text-sm font-semibold text-white shadow-xs transition-colors hover:bg-indigo-700 focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-2"
                >
                    Sign in
                </button>
            </form>
        </div>
    </main>
</div>
