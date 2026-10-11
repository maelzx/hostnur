<main class="mx-auto flex min-h-screen max-w-md flex-col justify-center px-6 py-12">
    <header class="mb-8">
        <p class="mb-3 text-sm font-semibold uppercase tracking-widest text-indigo-700">{{ config('app.name') }}</p>
        <h1 class="text-2xl font-semibold tracking-tight">Administrator sign in</h1>
        <p class="mt-2 text-sm leading-6 text-slate-600">
            Access is limited to the provisioned Hostnur administrator account.
        </p>
    </header>

    <form method="POST" action="{{ route('login.store') }}" class="space-y-5">
        @csrf

        @if ($errors->any())
            <p role="alert" class="rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-800">
                {{ $errors->first() }}
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
                required
                autofocus
                class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm shadow-sm focus:border-indigo-500 focus:outline-none focus:ring-1 focus:ring-indigo-500"
            >
        </div>

        <div>
            <label for="password" class="block text-sm font-medium text-slate-700">Password</label>
            <input
                id="password"
                name="password"
                type="password"
                autocomplete="current-password"
                required
                class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm shadow-sm focus:border-indigo-500 focus:outline-none focus:ring-1 focus:ring-indigo-500"
            >
        </div>

        <button
            type="submit"
            class="w-full rounded-lg bg-indigo-700 px-4 py-2 text-sm font-semibold text-white hover:bg-indigo-800 focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:ring-offset-2"
        >
            Sign in
        </button>
    </form>

    <p class="mt-6 text-sm leading-6 text-slate-600">
        No public registration. Account recovery and multi-factor authentication are
        upcoming slices and are not available yet.
    </p>
</main>
