<main class="mx-auto flex min-h-screen max-w-4xl flex-col justify-center px-6 py-12 sm:px-10">
    <header class="mb-10">
        <p class="mb-3 text-sm font-semibold uppercase tracking-widest text-indigo-700">{{ config('app.name') }}</p>
        <h1 class="text-3xl font-semibold tracking-tight sm:text-4xl">Your VPS control panel, taking shape.</h1>
        <p class="mt-4 max-w-2xl text-base leading-7 text-slate-600">
            The web foundation is ready. Hostnur will bring application deployment
            and everyday server administration into one native-first workspace.
        </p>
    </header>

    <section aria-labelledby="foundation-heading" class="rounded-2xl border border-slate-200 bg-white p-6 sm:p-8">
        <h2 id="foundation-heading" class="text-lg font-semibold">Development foundation</h2>
        <dl class="mt-6 grid gap-6 sm:grid-cols-2">
            <div>
                <dt class="text-sm text-slate-600">Web framework</dt>
                <dd class="mt-1 font-medium">Laravel + Livewire</dd>
            </div>
            <div>
                <dt class="text-sm text-slate-600">Interface</dt>
                <dd class="mt-1 font-medium">Blade + Tailwind CSS</dd>
            </div>
            <div>
                <dt class="text-sm text-slate-600">Panel persistence</dt>
                <dd class="mt-1 font-medium">SQLite + database-backed queue</dd>
            </div>
            <div>
                <dt class="text-sm text-slate-600">Rust agent</dt>
                <dd class="mt-1 font-medium text-amber-800">Not connected — planned for a later stage</dd>
            </div>
        </dl>
    </section>

    <footer class="mt-6 text-sm leading-6 text-slate-600">
        Development scaffold · Authentication and server operations are not available yet.
    </footer>
</main>
