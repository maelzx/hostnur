<main class="mx-auto flex min-h-screen max-w-4xl flex-col justify-center px-6 py-12 sm:px-10">
    <header class="mb-10">
        <p class="mb-3 text-sm font-semibold uppercase tracking-widest text-indigo-700">{{ config('app.name') }}</p>
        <h1 class="text-3xl font-semibold tracking-tight sm:text-4xl">Administrator panel</h1>
        <p class="mt-4 max-w-2xl text-base leading-7 text-slate-600">
            Signed in as {{ auth()->user()?->name }}. This restricted area confirms an
            administrator session; multi-factor authentication and administrative
            operations are not available yet.
        </p>
    </header>

    <section aria-labelledby="panel-status-heading" class="rounded-2xl border border-slate-200 bg-white p-6 sm:p-8">
        <h2 id="panel-status-heading" class="text-lg font-semibold">Current status</h2>
        <dl class="mt-6 grid gap-6 sm:grid-cols-2">
            <div>
                <dt class="text-sm text-slate-600">Administrator session</dt>
                <dd class="mt-1 font-medium text-emerald-800">Active</dd>
            </div>
            <div>
                <dt class="text-sm text-slate-600">Multi-factor authentication</dt>
                <dd class="mt-1 font-medium text-amber-800">Not enforced yet — upcoming slice</dd>
            </div>
            <div>
                <dt class="text-sm text-slate-600">Agent connection</dt>
                <dd class="mt-1 font-medium text-amber-800">Not connected — planned for a later stage</dd>
            </div>
            <div>
                <dt class="text-sm text-slate-600">Administrative operations</dt>
                <dd class="mt-1 font-medium text-amber-800">None available yet</dd>
            </div>
        </dl>
    </section>

    <form method="POST" action="{{ route('logout') }}" class="mt-6">
        @csrf
        <button
            type="submit"
            class="rounded-lg border border-slate-300 px-4 py-2 text-sm font-semibold text-slate-700 hover:bg-slate-100 focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:ring-offset-2"
        >
            Sign out
        </button>
    </form>
</main>
