<main>
    <header class="mb-6">
        <h1 class="text-2xl font-semibold tracking-tight">Applications</h1>
    </header>

    <x-ui.panel title="New application" class="mb-6">
        <form wire:submit="create" class="grid gap-4 sm:grid-cols-2">
            <div>
                <label for="name" class="block text-sm font-medium text-slate-700">Display name</label>
                <input id="name" type="text" wire:model="name" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm focus:border-indigo-500 focus:outline-none focus-visible:ring-1 focus-visible:ring-indigo-500">
                @error('name') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
            </div>
            <div>
                <label for="slug" class="block text-sm font-medium text-slate-700">Slug (immutable)</label>
                <input id="slug" type="text" wire:model="slug" placeholder="my-app" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm focus:border-indigo-500 focus:outline-none focus-visible:ring-1 focus-visible:ring-indigo-500">
                @error('slug') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
            </div>
            <div>
                <label for="provider" class="block text-sm font-medium text-slate-700">Provider</label>
                <select id="provider" wire:model="provider" class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm">
                    @foreach ($providers as $option)
                        <option value="{{ $option }}">{{ $option }}</option>
                    @endforeach
                </select>
                @error('provider') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
            </div>
            <div>
                <label for="runtime" class="block text-sm font-medium text-slate-700">Runtime</label>
                <select id="runtime" wire:model="runtime" class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm">
                    @foreach ($runtimes as $option)
                        <option value="{{ $option }}">{{ $option }}</option>
                    @endforeach
                </select>
                @error('runtime') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
            </div>
            <div>
                <label for="runtime_version" class="block text-sm font-medium text-slate-700">Runtime version</label>
                <input id="runtime_version" type="text" wire:model="runtime_version" placeholder="8.3" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm">
                @error('runtime_version') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
            </div>
            <div class="flex items-end">
                <x-ui.button type="submit">Create application</x-ui.button>
            </div>
        </form>
    </x-ui.panel>

    @if ($applications->isEmpty())
        <x-ui.empty-state title="No applications yet">Create your first application above.</x-ui.empty-state>
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
                            <th scope="col" class="px-4 py-3 font-medium">State</th>
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
                                    @if ($application->isArchived())
                                        <x-ui.badge tone="warning">archived</x-ui.badge>
                                    @else
                                        <x-ui.badge tone="info">desired</x-ui.badge>
                                    @endif
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
