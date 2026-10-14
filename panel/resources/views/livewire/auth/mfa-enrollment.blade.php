<div class="max-w-2xl">
    <h1 class="mb-8 text-2xl font-semibold tracking-tight text-slate-900">Multi-factor authentication</h1>

    @if ($user->two_factor_secret === null)
        <section class="rounded-xl border border-slate-200 bg-white p-6 shadow-xs">
            <h2 class="text-lg font-semibold tracking-tight text-slate-900">Setup required</h2>
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
                    Scan the QR code, then confirm with the code below.
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

            <div
                id="recovery-codes"
                x-data="{ copied: false, codes: @js($recoveryCodes) }"
                class="rounded-xl border border-slate-200 bg-white p-6 shadow-xs"
            >
                <div class="flex flex-wrap items-start justify-between gap-4">
                    <div class="max-w-md">
                        <h2 class="text-lg font-semibold tracking-tight text-slate-900">Recovery codes</h2>
                        <p class="mt-2 text-sm leading-6 text-slate-600">
                            Shown only once. Save them somewhere safe.
                        </p>
                    </div>

                    <div class="flex flex-wrap gap-2 print:hidden">
                        <button
                            type="button"
                            x-on:click="navigator.clipboard?.writeText(codes.join('\n')).then(() => { copied = true; setTimeout(() => copied = false, 2000) }).catch(() => { copied = false })"
                            class="inline-flex items-center justify-center gap-2 rounded-lg border border-slate-300 bg-white px-4 py-2 text-sm font-semibold text-slate-700 transition-colors hover:bg-slate-50 focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-2"
                        >
                            <svg x-show="! copied" class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect width="14" height="14" x="8" y="8" rx="2" ry="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/></svg>
                            <svg x-show="copied" x-cloak class="h-4 w-4 text-emerald-600" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M20 6 9 17l-5-5"/></svg>
                            <span x-show="! copied">Copy codes</span>
                            <span x-show="copied" x-cloak>Copied</span>
                        </button>

                        <button
                            type="button"
                            x-on:click="window.print()"
                            class="inline-flex items-center justify-center gap-2 rounded-lg border border-slate-300 bg-white px-4 py-2 text-sm font-semibold text-slate-700 transition-colors hover:bg-slate-50 focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-2"
                        >
                            <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M6 18H4a2 2 0 0 1-2-2v-5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2v5a2 2 0 0 1-2 2h-2"/><path d="M6 9V3a1 1 0 0 1 1-1h10a1 1 0 0 1 1 1v6"/><rect x="6" y="14" width="12" height="8" rx="1"/></svg>
                            Save as PDF
                        </button>
                    </div>
                </div>

                <p class="sr-only" role="status" aria-live="polite" x-text="copied ? 'Recovery codes copied to clipboard' : ''"></p>

                <p class="mt-4 hidden text-xs font-medium uppercase tracking-wide text-slate-500 print:block">
                    {{ config('app.name') }} · account recovery codes
                </p>

                <ul class="mt-4 grid grid-cols-2 gap-2 font-mono text-sm print:grid-cols-2 print:gap-3">
                    @foreach ($recoveryCodes as $recoveryCode)
                        <li class="rounded bg-slate-100 px-3 py-2 print:border print:border-slate-300 print:bg-white">{{ $recoveryCode }}</li>
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
