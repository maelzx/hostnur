<?php

namespace App\Agent\Dto;

use App\Agent\AgentContractException;

/**
 * Per-application storage observation. Unavailable values are null, never zero.
 */
final readonly class ApplicationMetricsSnapshot
{
    public function __construct(
        public int $protocolVersion,
        public string $applicationId,
        public ?int $storageUsedBytes,
        public ?int $storageTotalBytes,
        public string $sampledAt,
    ) {}

    /**
     * @param  array<string, mixed>  $data
     */
    public static function fromArray(array $data): self
    {
        $extra = array_diff(array_keys($data), [
            'protocol_version', 'application_id', 'storage_used_bytes', 'storage_total_bytes', 'sampled_at',
        ]);

        if ($extra !== []) {
            throw AgentContractException::invalidParameters(
                'Application metrics has unexpected keys: '.implode(', ', $extra).'.'
            );
        }

        $version = $data['protocol_version'] ?? null;
        $applicationId = $data['application_id'] ?? null;
        $used = $data['storage_used_bytes'] ?? null;
        $total = $data['storage_total_bytes'] ?? null;
        $sampledAt = $data['sampled_at'] ?? null;

        $validBytes = fn (mixed $value): bool => $value === null || (is_int($value) && $value >= 0);

        if (! is_int($version)
            || ! is_string($applicationId) || $applicationId === ''
            || ! $validBytes($used)
            || ! $validBytes($total)
            || ! is_string($sampledAt) || $sampledAt === '') {
            throw AgentContractException::invalidParameters('Application metrics snapshot is invalid.');
        }

        return new self($version, $applicationId, $used, $total, $sampledAt);
    }
}
