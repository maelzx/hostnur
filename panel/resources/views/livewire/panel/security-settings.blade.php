<div class="max-w-3xl">
    <header class="mb-6">
        <h1 class="text-2xl font-semibold tracking-tight">Security settings</h1>
        <p class="mt-2 text-sm leading-6 text-slate-600">
            Sensitive changes require a recent password confirmation. Rotating your
            password or multi-factor authentication signs out existing sessions.
        </p>
    </header>

    @if ($recoveryCodes !== [])
        <section class="mb-6 rounded-xl border border-amber-300 bg-amber-50 p-6">
            <h2 class="text-lg font-semibold text-amber-900">New recovery codes</h2>
            <p class="mt-2 text-sm leading-6 text-amber-900">
                Store these one-time codes now. They replace all previous recovery
                codes and are shown only once.
            </p>
            <ul class="mt-4 grid grid-cols-1 gap-2 font-mono text-sm sm:grid-cols-2">
                @foreach ($recoveryCodes as $recoveryCode)
                    <li class="rounded bg-white px-3 py-2">{{ $recoveryCode }}</li>
                @endforeach
            </ul>
        </section>
    @endif

    <x-ui.panel title="Change password" class="mb-6">
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
                    class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm shadow-sm focus:border-indigo-500 focus:outline-none focus-visible:ring-1 focus-visible:ring-indigo-500"
                >
            </div>

            <div>
                <label for="password_confirmation" class="block text-sm font-medium text-slate-700">Confirm new password</label>
                <input
                    id="password_confirmation"
                    type="password"
                    wire:model="password_confirmation"
                    autocomplete="new-password"
                    class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm shadow-sm focus:border-indigo-500 focus:outline-none focus-visible:ring-1 focus-visible:ring-indigo-500"
                >
            </div>

            <x-ui.button type="submit" wire:loading.attr="disabled">Change password</x-ui.button>
        </form>
    </x-ui.panel>

    <x-ui.panel title="Multi-factor authentication">
        @if ($user->hasEnabledTwoFactorAuthentication())
            <p class="mt-2 text-sm leading-6 text-slate-600">
                Multi-factor authentication is enabled. Disabling it removes the
                secret and all recovery codes and signs out existing sessions.
            </p>
            <div class="mt-4 flex flex-wrap gap-3">
                <x-ui.confirm
                    title="Regenerate recovery codes?"
                    confirm-label="Regenerate"
                    method="regenerateRecoveryCodes"
                    variant="secondary"
                >
                    <x-slot:trigger>Regenerate recovery codes</x-slot:trigger>
                    This replaces all existing recovery codes with a new set shown once.
                </x-ui.confirm>

                <x-ui.confirm
                    title="Disable multi-factor authentication?"
                    confirm-label="Disable MFA"
                    method="disableTwoFactor"
                    variant="danger"
                >
                    <x-slot:trigger>Disable multi-factor authentication</x-slot:trigger>
                    This removes the secret and recovery codes and signs out existing
                    sessions. It cannot be undone.
                </x-ui.confirm>
            </div>
        @else
            <p class="mt-2 text-sm leading-6 text-slate-600">
                Multi-factor authentication is not enabled.
            </p>
            <p class="mt-4">
                <a href="{{ route('panel.security.mfa') }}" class="font-medium text-indigo-700 hover:underline">Set up multi-factor authentication</a>
            </p>
        @endif
    </x-ui.panel>
</div>
