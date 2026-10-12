<main class="mx-auto flex min-h-screen max-w-md flex-col justify-center px-6 py-12">
    <header class="mb-8">
        <p class="mb-3 text-sm font-semibold uppercase tracking-widest text-indigo-700">{{ config('app.name') }}</p>
        <h1 class="text-2xl font-semibold tracking-tight">Confirm your password</h1>
        <p class="mt-2 text-sm leading-6 text-slate-600">
            Re-enter your account password to continue. This confirmation lasts a
            few minutes and is required before sensitive security changes.
        </p>
    </header>

    <form wire:submit="confirm" class="space-y-5">
        @error('password')
            <p role="alert" class="rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-800">{{ $message }}</p>
        @enderror

        <div>
            <label for="password" class="block text-sm font-medium text-slate-700">Password</label>
            <input
                id="password"
                type="password"
                wire:model="password"
                autocomplete="current-password"
                autofocus
                class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm shadow-sm focus:border-indigo-500 focus:outline-none focus:ring-1 focus:ring-indigo-500"
            >
        </div>

        <button
            type="submit"
            class="w-full rounded-lg bg-indigo-700 px-4 py-2 text-sm font-semibold text-white hover:bg-indigo-800 focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:ring-offset-2"
        >
            Confirm
        </button>
    </form>

    <p class="mt-6 text-center text-sm">
        <a href="{{ route('panel.dashboard') }}" class="font-medium text-slate-600 hover:underline">Cancel</a>
    </p>
</main>
