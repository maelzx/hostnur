<main>
    <header class="mb-6">
        <h1 class="text-2xl font-semibold tracking-tight">SSL certificates</h1>
    </header>

    <div class="grid gap-6 lg:grid-cols-2">
        <x-ui.panel title="Request managed certificate (HTTP-01)">
            <form wire:submit="requestManaged" class="mt-4 space-y-3">
                <div>
                    <label for="tls_domain" class="block text-sm font-medium text-slate-700">Domain</label>
                    <select id="tls_domain" wire:model="domainId" class="mt-1 block w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm">
                        <option value="">Select a configured domain</option>
                        @foreach ($domains as $domain)
                            <option value="{{ $domain->id }}">{{ $domain->hostname }}</option>
                        @endforeach
                    </select>
                    @error('hostname') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                </div>
                <x-ui.button type="submit" variant="secondary">Request issuance</x-ui.button>
            </form>
        </x-ui.panel>

        <x-ui.panel title="Upload certificate">
            <form wire:submit="ingest" class="mt-4 space-y-3">
                <div>
                    <label for="certificate_pem" class="block text-sm font-medium text-slate-700">Public certificate (PEM)</label>
                    <textarea id="certificate_pem" wire:model="certificate_pem" rows="4" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 font-mono text-xs"></textarea>
                    @error('certificate_pem') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                </div>
                <div>
                    <label for="private_key" class="block text-sm font-medium text-slate-700">Private key (PEM, encrypted at rest)</label>
                    <textarea id="private_key" wire:model="private_key" rows="4" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 font-mono text-xs"></textarea>
                    @error('private_key') <p role="alert" class="mt-1 text-sm text-red-700">{{ $message }}</p> @enderror
                </div>
                <div>
                    <label for="passphrase" class="block text-sm font-medium text-slate-700">Key passphrase (optional)</label>
                    <input id="passphrase" type="password" wire:model="passphrase" autocomplete="off" class="mt-1 block w-full rounded-lg border border-slate-300 px-3 py-2 text-sm">
                </div>
                <x-ui.button type="submit" variant="secondary">Validate and store</x-ui.button>
            </form>
        </x-ui.panel>
    </div>

    <x-ui.panel title="Certificates" class="mt-6">
        @if ($certificates->isEmpty())
            <p class="mt-4 text-sm text-slate-600">No certificates configured.</p>
        @else
            <div class="mt-4 overflow-x-auto">
                <table class="min-w-full divide-y divide-slate-200 text-sm">
                    <caption class="sr-only">Configured certificates</caption>
                    <thead class="bg-slate-50 text-left text-xs uppercase tracking-wide text-slate-500">
                        <tr>
                            <th scope="col" class="px-4 py-3 font-medium">Hostname</th>
                            <th scope="col" class="px-4 py-3 font-medium">Source</th>
                            <th scope="col" class="px-4 py-3 font-medium">State</th>
                            <th scope="col" class="px-4 py-3 font-medium">Expires</th>
                        </tr>
                    </thead>
                    <tbody class="divide-y divide-slate-100">
                        @foreach ($certificates as $certificate)
                            <tr wire:key="cert-{{ $certificate->id }}">
                                <td class="px-4 py-3 font-mono">{{ $certificate->hostname }}</td>
                                <td class="px-4 py-3 text-slate-600">{{ $certificate->source->value }}</td>
                                <td class="px-4 py-3"><x-ui.request-status :state="$certificate->state" /></td>
                                <td class="px-4 py-3 text-slate-600">{{ $certificate->expires_at?->toDateString() ?? '—' }}</td>
                            </tr>
                        @endforeach
                    </tbody>
                </table>
            </div>
        @endif
    </x-ui.panel>
</main>
