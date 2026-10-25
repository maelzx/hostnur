<?php

namespace App\Livewire\Panel;

use App\Models\ApplicationDomain;
use App\Models\Certificate;
use App\Models\User;
use App\Services\CertificateService;
use Illuminate\Contracts\View\View;
use Illuminate\Support\Str;
use Livewire\Attributes\Layout;
use Livewire\Component;

#[Layout('layouts.panel')]
class CertificateIndex extends Component
{
    public string $domainId = '';

    public string $certificate_pem = '';

    public string $private_key = '';

    public string $passphrase = '';

    public string $managedKey = '';

    public string $ingestKey = '';

    public function mount(): void
    {
        $this->managedKey = (string) Str::uuid();
        $this->ingestKey = (string) Str::uuid();
    }

    public function requestManaged(CertificateService $service): void
    {
        $domain = $this->domain();
        $service->requestManaged($this->actor(), $domain, $this->managedKey);

        session()->flash('status', 'Certificate issuance requested (HTTP-01).');
        $this->redirect(route('panel.certificates.index'));
    }

    public function ingest(CertificateService $service): void
    {
        $domain = $this->domain();
        $service->ingestUserSupplied(
            $this->actor(),
            $domain,
            $this->certificate_pem,
            $this->private_key,
            $this->passphrase !== '' ? $this->passphrase : null,
            $this->ingestKey,
        );

        $this->reset('certificate_pem', 'private_key', 'passphrase');
        session()->flash('status', 'Certificate stored (the key is encrypted and write-only); installation requested.');
        $this->redirect(route('panel.certificates.index'));
    }

    public function render(CertificateService $service): View
    {
        $certificates = Certificate::query()->with('domain.application')->latest('created_at')->get()
            ->map(fn (Certificate $certificate): Certificate => $service->syncState($certificate));

        return view('livewire.panel.certificate-index', [
            'domains' => ApplicationDomain::query()->with('application')->orderBy('hostname')->get(),
            'certificates' => $certificates,
        ]);
    }

    private function domain(): ApplicationDomain
    {
        $domain = ApplicationDomain::query()->with('application')->findOrFail($this->domainId);
        $this->authorize('update', $domain->application);

        return $domain;
    }

    private function actor(): User
    {
        return auth()->user();
    }
}
