<main>
    <header class="mb-6 flex flex-wrap items-center justify-between gap-3">
        <h1 class="text-2xl font-semibold tracking-tight">Applications</h1>
        <a
            href="{{ route('panel.applications.create') }}"
            class="inline-flex items-center justify-center gap-2 rounded-lg bg-indigo-600 px-4 py-2 text-sm font-semibold text-white shadow-xs transition-colors hover:bg-indigo-700 focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-2"
        >
            Create new application
        </a>
    </header>

    @if ($applications->isEmpty())
        <x-ui.empty-state title="No applications yet">Create your first application to get started.</x-ui.empty-state>
    @else
        <section class="overflow-hidden rounded-xl border border-slate-200 bg-white shadow-xs">
            <div class="overflow-x-auto">
                <table class="min-w-full divide-y divide-slate-200 text-sm">
                    <caption class="sr-only">Managed applications</caption>
                    <thead class="bg-slate-50 text-left text-xs uppercase tracking-wide text-slate-500">
                        <tr>
                            <th scope="col" class="px-4 py-3 font-medium">Name</th>
                            <th scope="col" class="px-4 py-3 font-medium">Runtime</th>
                            <th scope="col" class="px-4 py-3 font-medium">Domains</th>
                            <th scope="col" class="px-4 py-3 font-medium">Status</th>
                        </tr>
                    </thead>
                    <tbody class="divide-y divide-slate-100">
                        @foreach ($applications as $application)
                            <tr wire:key="app-{{ $application->id }}">
                                <th scope="row" class="px-4 py-3 text-left font-normal">
                                    <a href="{{ route('panel.applications.show', ['application' => $application->id]) }}" class="font-medium text-indigo-700 hover:underline">{{ $application->name }}</a>
                                    <span class="block text-xs text-slate-500">{{ $application->slug }}</span>
                                </th>
                                <td class="px-4 py-3 text-slate-600">{{ $application->runtime }} {{ $application->runtime_version }}</td>
                                <td class="px-4 py-3 text-slate-600">{{ $application->domains_count }}</td>
                                <td class="px-4 py-3">
                                    <x-ui.application-status :status="$application->status()" :deployment="$application->latestDeployment" />
                                </td>
                            </tr>
                        @endforeach
                    </tbody>
                </table>
            </div>
        </section>

        <div class="mt-6">{{ $applications->links() }}</div>
    @endif
</main>
