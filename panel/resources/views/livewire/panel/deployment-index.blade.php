<main>
    <header class="mb-6">
        <h1 class="text-2xl font-semibold tracking-tight">Deployments</h1>
        <p class="mt-2 text-sm leading-6 text-slate-600">
            Deployment intent and history. Simulated runs are labelled; a simulated
            success never replaces a real active release.
        </p>
    </header>

    @if ($deployments->isEmpty())
        <x-ui.empty-state title="No deployments yet">Request a deployment from an application's operations page.</x-ui.empty-state>
    @else
        <section class="overflow-hidden rounded-xl border border-slate-200 bg-white shadow-xs">
            <div class="overflow-x-auto">
                <table class="min-w-full divide-y divide-slate-200 text-sm">
                    <caption class="sr-only">Deployment history</caption>
                    <thead class="bg-slate-50 text-left text-xs uppercase tracking-wide text-slate-500">
                        <tr>
                            <th scope="col" class="px-4 py-3 font-medium">Application</th>
                            <th scope="col" class="px-4 py-3 font-medium">Operation</th>
                            <th scope="col" class="px-4 py-3 font-medium">State</th>
                            <th scope="col" class="px-4 py-3 font-medium">Mode</th>
                            <th scope="col" class="px-4 py-3 font-medium">Requested</th>
                        </tr>
                    </thead>
                    <tbody class="divide-y divide-slate-100">
                        @foreach ($deployments as $deployment)
                            <tr wire:key="dep-{{ $deployment->id }}">
                                <th scope="row" class="px-4 py-3 text-left font-normal">
                                    <a href="{{ route('panel.deployments.show', ['deployment' => $deployment->id]) }}" class="font-medium text-indigo-700 hover:underline">{{ $deployment->application?->name ?? '—' }}</a>
                                </th>
                                <td class="px-4 py-3 text-slate-600">{{ $deployment->operation }}</td>
                                <td class="px-4 py-3"><x-ui.badge :tone="$deployment->state->value === 'failed' ? 'danger' : ($deployment->state->value === 'succeeded' ? 'success' : ($deployment->state->value === 'unknown' ? 'warning' : 'info'))">{{ $deployment->state->label() }}</x-ui.badge></td>
                                <td class="px-4 py-3">{{ $deployment->isSimulated() ? 'simulated' : $deployment->mode }}</td>
                                <td class="px-4 py-3 text-slate-600">{{ $deployment->requested_at?->toDayDateTimeString() ?? '—' }}</td>
                            </tr>
                        @endforeach
                    </tbody>
                </table>
            </div>
        </section>

        <div class="mt-6">{{ $deployments->links() }}</div>
    @endif
</main>
