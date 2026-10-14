<div class="max-w-2xl">
    <header class="mb-8">
        <h1 class="text-2xl font-semibold tracking-tight">Multi-factor authentication</h1>
        <p class="mt-2 text-sm leading-6 text-slate-600">
            Protect the administrator account with a time-based one-time password
            from an authenticator app. Secrets are stored encrypted and shown only
            during setup.
        </p>
    </header>

    @if ($user->two_factor_secret === null)
        <section class="rounded-xl border border-slate-200 bg-white p-6 shadow-xs">
            <h2 class="text-lg font-semibold tracking-tight text-slate-900">Setup required</h2>
            <p class="mt-2 text-sm leading-6 text-slate-600">
                Multi-factor authentication is not configured for this account yet.
            </p>
            <button
                type="button"
                wire:click="enable"
                wire:loading.attr="disabled"
                class="mt-4 rounded-lg bg-indigo-600 px-4 py-2 text-sm font-semibold text-white shadow-xs hover:bg-indigo-700 focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:ring-offset-2"
            >
                Begin setup
            </button>
        </section>
    @elseif ($pending)
        <section class="space-y-6">
            <div class="rounded-xl border border-slate-200 bg-white p-6 shadow-xs">
                <h2 class="text-lg font-semibold tracking-tight text-slate-900">Set up your authenticator</h2>
                <p class="mt-2 text-sm leading-6 text-slate-600">
                    Scan the QR code or enter the key manually in your authenticator
                    app. Then enter the generated six-digit code below to finish.
                </p>
                <div class="mt-4 flex flex-col gap-4 sm:flex-row sm:items-start">
                    <div class="h-48 w-48 shrink-0 [&>svg]:h-48 [&>svg]:w-48" aria-hidden="true">
                        {!! $qrSvg !!}
                    </div>
                    <div class="min-w-0">
                        <p class="text-sm text-slate-600">Setup key</p>
                        <code class="mt-1 block break-all rounded-lg bg-slate-100 px-3 py-2 font-mono text-sm tracking-wider">{{ $secret }}</code>
                    </div>
                </div>

                <form wire:submit="confirm" class="mt-6 border-t border-slate-200 pt-6">
                    <h3 class="text-base font-semibold tracking-tight text-slate-900">Confirm setup</h3>
                    <p class="mt-1 text-sm leading-6 text-slate-600">
                        Enter the current six-digit code from your authenticator app.
                    </p>

                    @error('code')
                        <p role="alert" class="mt-3 rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-800">{{ $message }}</p>
                    @enderror

                    <div class="mt-4 flex flex-wrap items-end gap-3">
                        <div>
                            <label for="code" class="block text-sm font-medium text-slate-700">Authentication code</label>
                            <input
                                id="code"
                                type="text"
                                wire:model="code"
                                inputmode="numeric"
                                autocomplete="one-time-code"
                                maxlength="6"
                                required
                                class="mt-1.5 block w-40 rounded-lg border border-slate-300 bg-white px-3.5 py-2.5 font-mono text-sm tracking-widest text-slate-900 shadow-xs transition-colors focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30"
                            >
                        </div>
                        <button
                            type="submit"
                            class="inline-flex items-center justify-center gap-2 rounded-lg bg-indigo-600 px-4 py-2.5 text-sm font-semibold text-white shadow-xs transition-colors hover:bg-indigo-700 focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-2"
                        >
                            Confirm
                        </button>
                    </div>
                </form>
            </div>

            <div class="rounded-xl border border-slate-200 bg-white p-6 shadow-xs">
                <h2 class="text-lg font-semibold tracking-tight text-slate-900">Recovery codes</h2>
                <p class="mt-2 text-sm leading-6 text-slate-600">
                    Store these one-time codes somewhere safe. They are shown only
                    during setup and each can be used once if you lose your device.
                </p>
                <ul class="mt-4 grid grid-cols-2 gap-2 font-mono text-sm">
                    @foreach ($recoveryCodes as $recoveryCode)
                        <li class="rounded bg-slate-100 px-3 py-2">{{ $recoveryCode }}</li>
                    @endforeach
                </ul>
            </div>
        </section>
    @else
        <section class="rounded-xl border border-emerald-200 bg-emerald-50 p-6">
            <h2 class="text-lg font-semibold text-emerald-900">Enabled and confirmed</h2>
            <p class="mt-2 text-sm leading-6 text-emerald-800">
                Multi-factor authentication was confirmed on
                {{ $user->two_factor_confirmed_at?->toDayDateTimeString() }}.
            </p>
        </section>
    @endif

    <p class="mt-6 text-sm leading-6 text-slate-600">
        <a href="{{ route('panel.dashboard') }}" class="font-medium text-indigo-700 hover:underline">Back to panel</a>
    </p>
</div>
