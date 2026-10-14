<main class="flex min-h-dvh items-center justify-center px-6 py-12">
    <div class="w-full max-w-sm">
        <div class="mb-8 flex items-center gap-3">
            <span class="inline-flex h-10 w-10 items-center justify-center rounded-xl bg-indigo-600 text-lg font-bold text-white" aria-hidden="true">H</span>
            <span class="text-lg font-semibold tracking-tight text-slate-900">{{ config('app.name') }}</span>
        </div>

        <header class="mb-8">
            <h1 class="text-2xl font-semibold tracking-tight text-slate-900">Two-factor verification</h1>
            <p class="mt-2 text-sm leading-6 text-slate-600">
                Enter the current six-digit code from your authenticator app, or use one
                of your recovery codes.
            </p>
        </header>

        <form method="POST" action="{{ route('two-factor.login.store') }}" class="space-y-5">
            @csrf

            @foreach (['code', 'recovery_code'] as $field)
                @error($field)
                    <p role="alert" class="flex items-start gap-2 rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-800">
                        <svg class="mt-0.5 h-4 w-4 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="12" cy="12" r="10"/><path d="M12 8v4"/><path d="M12 16h.01"/></svg>
                        <span>{{ $message }}</span>
                    </p>
                @enderror
            @endforeach

            <div>
                <label for="code" class="block text-sm font-medium text-slate-700">Authentication code</label>
                <input
                    id="code"
                    name="code"
                    type="text"
                    inputmode="numeric"
                    autocomplete="one-time-code"
                    maxlength="6"
                    autofocus
                    class="mt-1.5 block w-full rounded-lg border border-slate-300 bg-white px-3.5 py-2.5 font-mono text-sm tracking-widest text-slate-900 shadow-xs transition-colors focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30"
                >
            </div>

            <details class="rounded-lg border border-slate-200 bg-white p-4">
                <summary class="cursor-pointer text-sm font-medium text-slate-700">Use a recovery code</summary>
                <div class="mt-3">
                    <label for="recovery_code" class="block text-sm font-medium text-slate-700">Recovery code</label>
                    <input
                        id="recovery_code"
                        name="recovery_code"
                        type="text"
                        autocomplete="one-time-code"
                        class="mt-1.5 block w-full rounded-lg border border-slate-300 bg-white px-3.5 py-2.5 font-mono text-sm text-slate-900 shadow-xs transition-colors focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30"
                    >
                </div>
            </details>

            <button
                type="submit"
                class="inline-flex w-full items-center justify-center gap-2 rounded-lg bg-indigo-600 px-4 py-2.5 text-sm font-semibold text-white shadow-xs transition-colors hover:bg-indigo-700 focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-2"
            >
                Verify
            </button>
        </form>

        <p class="mt-6 text-center text-sm">
            <a href="{{ route('login') }}" class="font-medium text-slate-600 hover:text-slate-900 hover:underline">Cancel and return to sign in</a>
        </p>
    </div>
</main>
