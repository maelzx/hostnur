<?php

namespace App\Services;

use App\Agent\Dto\OperationRequest;
use App\Agent\Dto\ResourceRef;
use App\Contracts\AgentClient;
use App\Enums\AuditAction;
use App\Enums\AuditOutcome;
use App\Enums\CertificateSource;
use App\Enums\RequestState;
use App\Enums\ResourceKind;
use App\Models\ApplicationDomain;
use App\Models\Certificate;
use App\Models\Task;
use App\Models\User;
use Illuminate\Support\Carbon;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Str;
use Illuminate\Validation\ValidationException;

/**
 * Certificate configuration only. User-supplied PEM/key material is validated
 * locally with OpenSSL, stored with the key encrypted and hidden, and never given
 * a public URL. Managed issuance is an HTTP-01 task; no wildcard/DNS-01 claim.
 */
final class CertificateService
{
    public function __construct(
        private readonly TaskService $tasks,
        private readonly AuditRecorder $audit,
        private readonly AgentClient $agent,
    ) {}

    public function requestManaged(User $actor, ApplicationDomain $domain, string $idempotencyKey): Certificate
    {
        return DB::transaction(function () use ($actor, $domain, $idempotencyKey): Certificate {
            if (str_starts_with($domain->hostname, '*')) {
                throw ValidationException::withMessages(['hostname' => 'Wildcard certificates are not supported.']);
            }

            $certificate = Certificate::create([
                'application_domain_id' => $domain->getKey(),
                'hostname' => $domain->hostname,
                'source' => CertificateSource::ManagedHttp01,
                'provider' => 'http01',
                'state' => RequestState::Requested,
            ]);

            $this->registerWithAgent($domain);

            $task = $this->tasks->requestApproved($actor, new OperationRequest(
                protocolVersion: (int) config('hostnur.protocol_version', 1),
                requestId: (string) Str::uuid(),
                idempotencyKey: $idempotencyKey,
                operation: 'certificate.issue',
                resource: new ResourceRef(ResourceKind::Domain, $domain->getKey()),
                parameters: ['provider' => 'http01'],
                timeoutSeconds: 300,
            ));

            $certificate->forceFill(['task_id' => $task->getKey()])->save();

            $this->audit->record($actor, AuditAction::CertificateIssueRequested, AuditOutcome::Requested, [
                'resource_kind' => 'application',
                'resource_id' => $domain->application_id,
                'metadata' => ['operation' => 'certificate.issue'],
            ]);

            return $certificate;
        });
    }

    public function ingestUserSupplied(User $actor, ApplicationDomain $domain, string $pem, string $key, ?string $passphrase): Certificate
    {
        $inspected = $this->inspect($pem, $key, $passphrase, $domain->hostname);

        $certificate = Certificate::create([
            'application_domain_id' => $domain->getKey(),
            'hostname' => $domain->hostname,
            'source' => CertificateSource::UserSupplied,
            'certificate_pem' => $pem,
            'private_key' => $key,
            'passphrase' => $passphrase,
            'issued_at' => $inspected['issued_at'],
            'expires_at' => $inspected['expires_at'],
            'state' => RequestState::Succeeded,
        ]);

        $this->audit->record($actor, AuditAction::CertificateIngested, AuditOutcome::Success, [
            'resource_kind' => 'application',
            'resource_id' => $domain->application_id,
            'metadata' => ['operation' => 'certificate.ingest'],
        ]);

        return $certificate;
    }

    public function syncState(Certificate $certificate): Certificate
    {
        if ($certificate->task === null || $certificate->task->status->value !== 'succeeded') {
            if ($certificate->task !== null) {
                $certificate->forceFill([
                    'state' => RequestState::fromTaskStatus($certificate->task->status),
                    'observed_state' => $certificate->task->status->value,
                ])->save();
            }

            return $certificate;
        }

        if ($certificate->state !== RequestState::Succeeded) {
            $certificate->forceFill([
                'state' => RequestState::Succeeded,
                'observed_state' => 'issued',
                'observed_provenance' => 'fake',
                'expires_at' => $certificate->expires_at ?? now()->addDays(90),
            ])->save();
        }

        return $certificate;
    }

    /**
     * @return array{issued_at: Carbon|null, expires_at: Carbon|null}
     */
    private function inspect(string $pem, string $key, ?string $passphrase, string $hostname): array
    {
        $max = (int) config('hostnur.server.certificate_max_bytes', 65536);

        if ($pem === '' || strlen($pem) > $max || strlen($key) > $max) {
            throw ValidationException::withMessages(['certificate_pem' => 'Provide a bounded PEM certificate and key.']);
        }

        $certificate = openssl_x509_read($pem);

        if ($certificate === false) {
            throw ValidationException::withMessages(['certificate_pem' => 'The public certificate could not be parsed.']);
        }

        $privateKey = openssl_pkey_get_private($key, $passphrase ?? '');

        if ($privateKey === false) {
            throw ValidationException::withMessages(['private_key' => 'The private key could not be parsed or the passphrase is wrong.']);
        }

        if (! openssl_x509_check_private_key($certificate, $privateKey)) {
            throw ValidationException::withMessages(['private_key' => 'The private key does not match the certificate.']);
        }

        $parsed = openssl_x509_parse($certificate);
        $names = $this->certificateNames($parsed);

        if (! in_array(strtolower($hostname), $names, true)) {
            throw ValidationException::withMessages(['certificate_pem' => 'The certificate does not cover this hostname.']);
        }

        $issuedAt = isset($parsed['validFrom_time_t']) ? Carbon::createFromTimestamp($parsed['validFrom_time_t']) : null;
        $expiresAt = isset($parsed['validTo_time_t']) ? Carbon::createFromTimestamp($parsed['validTo_time_t']) : null;

        if ($expiresAt === null || $expiresAt->isPast()) {
            throw ValidationException::withMessages(['certificate_pem' => 'The certificate is already expired.']);
        }

        return ['issued_at' => $issuedAt, 'expires_at' => $expiresAt];
    }

    /**
     * @param  array<string, mixed>  $parsed
     * @return list<string>
     */
    private function certificateNames(array $parsed): array
    {
        $names = [];

        if (isset($parsed['subject']['CN'])) {
            $names[] = strtolower((string) $parsed['subject']['CN']);
        }

        $san = $parsed['extensions']['subjectAltName'] ?? '';

        foreach (explode(',', (string) $san) as $entry) {
            $entry = trim($entry);

            if (str_starts_with($entry, 'DNS:')) {
                $names[] = strtolower(substr($entry, 4));
            }
        }

        return $names;
    }

    /**
     * Register the certificate request (domain id + hostname + application) with
     * the agent so `certificate.issue` can resolve the domain. Real mode only.
     */
    private function registerWithAgent(ApplicationDomain $domain): void
    {
        if (config('hostnur.agent_mode') !== 'real') {
            return;
        }

        try {
            $this->agent->registerCertificate(new OperationRequest(
                protocolVersion: (int) config('hostnur.protocol_version', 1),
                requestId: (string) Str::uuid(),
                idempotencyKey: (string) Str::uuid(),
                operation: 'agent.register_certificate',
                resource: null,
                parameters: [
                    'certificate' => (string) $domain->getKey(),
                    'hostname' => $domain->hostname,
                    'application' => $domain->application_id,
                ],
                timeoutSeconds: null,
            ));
        } catch (\Throwable $exception) {
            throw ValidationException::withMessages([
                'hostname' => 'Could not register the certificate with the agent: '.$exception->getMessage(),
            ]);
        }
    }
}
