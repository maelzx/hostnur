<main class="mx-auto flex min-h-screen max-w-3xl flex-col justify-center px-6 py-12">
    <header class="mb-8">
        <p class="mb-3 text-sm font-semibold uppercase tracking-widest text-indigo-700">{{ config('app.name') }}</p>
        <h1 class="text-2xl font-semibold tracking-tight">Security settings</h1>
        <p class="mt-2 text-sm leading-6 text-slate-600">
            Sensitive changes require a recent password confirmation. Rotating your
            password or multi-factor authentication signs out existing sessions.
        </p>
    </header>

    @if ($recoveryCodes !== [])
        <section class="mb-6 rounded-2xl border border-amber-300 bg-amber-50 p-6">
            <h2 class="text-lg font-semibold text-amber-900">New recovery codes</h2>
            <p class="mt-2 text-sm leading-6 text-amber-900">
                Store these one-time codes now. They replace all previous recovery
                codes and are shown only once.
            </p>
            <ul class="mt-4 grid grid-cols-2 gap-2 font-mono text-sm">
                @foreach ($recoveryCodes as $recoveryCode)
                    <li class="rounded bg-white px-3 py-2">{{ $recoveryCode }}</li>
                @endforeach
            </ul>
        </section>
    @endif

    <section class="mb-6 rounded-2xl border border-slate-200 bg-white p-6">
        <h2 class="text-lg font-semibold">Change password</h2>
        <p class="mt-2 text-sm leading-6 text-slate-600">
            Use at least 12 characters and at most 72 bytes.
        </p>

        <form wire:submit="changePassword" class="mt-4 space-y-4">
            @error('password')
                <p role="alert" class="rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-800">{{ $message }}</p>
            @enderror

            <div>
                <label for="password" class="block text-sm font-medium text-slate-700">New password</label>
                <input
                    id="password"
                    type="password"
                    wire:model="password"
                    autocomplete="new-password"
                    class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm shadow-sm focus:border-indigo-500 focus:outline-none focus:ring-1 focus:ring-indigo-500"
                >
            </div>

            <div>
                <label for="password_confirmation" class="block text-sm font-medium text-slate-700">Confirm new password</label>
                <input
                    id="password_confirmation"
                    type="password"
                    wire:model="password_confirmation"
                    autocomplete="new-password"
                    class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm shadow-sm focus:border-indigo-500 focus:outline-none focus:ring-1 focus:ring-indigo-500"
                >
            </div>

            <button
                type="submit"
                wire:loading.attr="disabled"
                class="rounded-lg bg-indigo-700 px-4 py-2 text-sm font-semibold text-white hover:bg-indigo-800 focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:ring-offset-2"
            >
                Change password
            </button>
        </form>
    </section>

    <section class="rounded-2xl border border-slate-200 bg-white p-6">
        <h2 class="text-lg font-semibold">Multi-factor authentication</h2>

        @if ($user->hasEnabledTwoFactorAuthentication())
            <p class="mt-2 text-sm leading-6 text-slate-600">
                Multi-factor authentication is enabled. Disabling it removes the
                secret and all recovery codes and signs out existing sessions.
            </p>
            <div class="mt-4 flex flex-wrap gap-3">
                <button
                    type="button"
                    wire:click="regenerateRecoveryCodes"
                    wire:loading.attr="disabled"
                    class="rounded-lg border border-slate-300 px-4 py-2 text-sm font-semibold text-slate-700 hover:bg-slate-100 focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:ring-offset-2"
                >
                    Regenerate recovery codes
                </button>
                <button
                    type="button"
                    wire:click="disableTwoFactor"
                    wire:loading.attr="disabled"
                    class="rounded-lg border border-red-300 px-4 py-2 text-sm font-semibold text-red-700 hover:bg-red-50 focus:outline-none focus:ring-2 focus:ring-red-500 focus:ring-offset-2"
                >
                    Disable multi-factor authentication
                </button>
            </div>
        @else
            <p class="mt-2 text-sm leading-6 text-slate-600">
                Multi-factor authentication is not enabled.
            </p>
            <p class="mt-4">
                <a href="{{ route('panel.security.mfa') }}" class="font-medium text-indigo-700 hover:underline">Set up multi-factor authentication</a>
            </p>
        @endif
    </section>

    <p class="mt-6 text-sm">
        <a href="{{ route('panel.dashboard') }}" class="font-medium text-indigo-700 hover:underline">Back to panel</a>
    </p>
</main>
