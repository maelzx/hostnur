<?php

namespace App\Agent\Dto;

use App\Agent\AgentContractException;

/**
 * A bounded, redacted tail of an application's own log files.
 */
final readonly class ApplicationLogSnapshot
{
    /**
     * @param  list<string>  $lines
     */
    public function __construct(
        public int $protocolVersion,
        public string $applicationId,
        public array $lines,
        public bool $truncated,
        public string $sampledAt,
    ) {}

    /**
     * @param  array<string, mixed>  $data
     */
    public static function fromArray(array $data): self
    {
        $extra = array_diff(array_keys($data), ['protocol_version', 'application_id', 'lines', 'truncated', 'sampled_at']);

        if ($extra !== []) {
            throw AgentContractException::invalidParameters(
                'Application log has unexpected keys: '.implode(', ', $extra).'.'
            );
        }

        $version = $data['protocol_version'] ?? null;
        $applicationId = $data['application_id'] ?? null;
        $lines = $data['lines'] ?? null;
        $truncated = $data['truncated'] ?? null;
        $sampledAt = $data['sampled_at'] ?? null;

        if (! is_int($version)
            || ! is_string($applicationId) || $applicationId === ''
            || ! is_bool($truncated)
            || ! is_string($sampledAt) || $sampledAt === '') {
            throw AgentContractException::invalidParameters('Application log snapshot is invalid.');
        }

        if (! is_array($lines) || count($lines) > 1000) {
            throw AgentContractException::invalidParameters('Application log lines must be a bounded array.');
        }

        foreach ($lines as $line) {
            if (! is_string($line)) {
                throw AgentContractException::invalidParameters('Each application log line must be a string.');
            }
        }

        return new self($version, $applicationId, array_values($lines), $truncated, $sampledAt);
    }
}
