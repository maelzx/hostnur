<?php

namespace App\Agent\Dto;

use App\Agent\AgentContractException;
use App\Enums\AgentMode;

/**
 * Agent-observed application lifecycle and active release (phase 20.5). This is
 * evidence from the host, distinct from the panel's desired configuration.
 */
final readonly class ApplicationObservation
{
    public const LIFECYCLES = ['reserved', 'account_created', 'layout_prepared', 'ready', 'failed', 'drifted', 'unknown'];

    public function __construct(
        public int $protocolVersion,
        public string $applicationId,
        public string $lifecycle,
        public ?string $activeRelease,
        public AgentMode $mode,
        public string $sampledAt,
    ) {}

    /**
     * @param  array<string, mixed>  $data
     */
    public static function fromArray(array $data): self
    {
        $extra = array_diff(array_keys($data), [
            'protocol_version', 'application_id', 'lifecycle', 'active_release', 'mode', 'sampled_at',
        ]);

        if ($extra !== []) {
            throw AgentContractException::invalidParameters(
                'Application observation has unexpected keys: '.implode(', ', $extra).'.'
            );
        }

        $version = $data['protocol_version'] ?? null;
        $applicationId = $data['application_id'] ?? null;
        $lifecycle = $data['lifecycle'] ?? null;
        $activeRelease = $data['active_release'] ?? null;
        $mode = $data['mode'] ?? null;
        $mode = is_string($mode) ? AgentMode::tryFrom($mode) : null;
        $sampledAt = $data['sampled_at'] ?? null;

        if (! is_int($version)
            || ! is_string($applicationId) || $applicationId === ''
            || ! is_string($lifecycle) || ! in_array($lifecycle, self::LIFECYCLES, true)
            || ($activeRelease !== null && ! is_string($activeRelease))
            || $mode === null
            || ! is_string($sampledAt) || $sampledAt === '') {
            throw AgentContractException::invalidParameters('Application observation is invalid.');
        }

        return new self($version, $applicationId, $lifecycle, $activeRelease, $mode, $sampledAt);
    }
}
