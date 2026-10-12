<main class="mx-auto flex min-h-screen max-w-md flex-col justify-center px-6 py-12">
    <header class="mb-8">
        <p class="mb-3 text-sm font-semibold uppercase tracking-widest text-indigo-700">{{ config('app.name') }}</p>
        <h1 class="text-2xl font-semibold tracking-tight">Two-factor verification</h1>
        <p class="mt-2 text-sm leading-6 text-slate-600">
            Enter the current six-digit code from your authenticator app, or use one
            of your recovery codes.
        </p>
    </header>

    <form method="POST" action="{{ route('two-factor.login.store') }}" class="space-y-5">
        @csrf

        @foreach (['code', 'recovery_code'] as $field)
            @error($field)
                <p role="alert" class="rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-800">{{ $message }}</p>
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
                class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 font-mono text-sm tracking-widest shadow-sm focus:border-indigo-500 focus:outline-none focus:ring-1 focus:ring-indigo-500"
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
                    class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 font-mono text-sm shadow-sm focus:border-indigo-500 focus:outline-none focus:ring-1 focus:ring-indigo-500"
                >
            </div>
        </details>

        <button
            type="submit"
            class="w-full rounded-lg bg-indigo-700 px-4 py-2 text-sm font-semibold text-white hover:bg-indigo-800 focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:ring-offset-2"
        >
            Verify
        </button>
    </form>

    <p class="mt-6 text-center text-sm">
        <a href="{{ route('login') }}" class="font-medium text-slate-600 hover:underline">Cancel and return to sign in</a>
    </p>
</main>
