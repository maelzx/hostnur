<?php

namespace App\Agent\Dto;

use App\Agent\AgentContractException;

/**
 * Pending package updates reported by the agent (phase 20.7).
 */
final readonly class ServerUpdatesSnapshot
{
    /**
     * @param  list<array{package: string, current: string, candidate: string, security: bool}>  $updates
     */
    public function __construct(
        public int $protocolVersion,
        public array $updates,
        public bool $truncated,
        public string $sampledAt,
    ) {}

    /**
     * @param  array<string, mixed>  $data
     */
    public static function fromArray(array $data): self
    {
        $extra = array_diff(array_keys($data), ['protocol_version', 'updates', 'truncated', 'sampled_at']);

        if ($extra !== []) {
            throw AgentContractException::invalidParameters(
                'Server updates has unexpected keys: '.implode(', ', $extra).'.'
            );
        }

        $version = $data['protocol_version'] ?? null;
        $updates = $data['updates'] ?? null;
        $truncated = $data['truncated'] ?? null;
        $sampledAt = $data['sampled_at'] ?? null;

        if (! is_int($version) || ! is_bool($truncated) || ! is_string($sampledAt) || $sampledAt === '') {
            throw AgentContractException::invalidParameters('Server updates snapshot is invalid.');
        }

        if (! is_array($updates) || count($updates) > 200) {
            throw AgentContractException::invalidParameters('Server updates must be a bounded array.');
        }

        $normalized = [];

        foreach ($updates as $entry) {
            if (! is_array($entry)) {
                throw AgentContractException::invalidParameters('Each update must be an object.');
            }

            $entryExtra = array_diff(array_keys($entry), ['package', 'current', 'candidate', 'security']);

            if ($entryExtra !== []) {
                throw AgentContractException::invalidParameters(
                    'Update has unexpected keys: '.implode(', ', $entryExtra).'.'
                );
            }

            $package = $entry['package'] ?? null;
            $current = $entry['current'] ?? null;
            $candidate = $entry['candidate'] ?? null;
            $security = $entry['security'] ?? null;

            if (! is_string($package) || $package === ''
                || ! is_string($current)
                || ! is_string($candidate) || $candidate === ''
                || ! is_bool($security)) {
                throw AgentContractException::invalidParameters('Update entry is invalid.');
            }

            $normalized[] = [
                'package' => $package,
                'current' => $current,
                'candidate' => $candidate,
                'security' => $security,
            ];
        }

        return new self($version, $normalized, $truncated, $sampledAt);
    }
}
