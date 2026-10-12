<?php

namespace App\Agent\Dto;

use App\Agent\AgentContractException;
use App\Enums\AgentMode;

final readonly class CapabilitySnapshot
{
    /**
     * @param  list<string>  $operations
     */
    public function __construct(
        public int $protocolVersion,
        public AgentMode $mode,
        public bool $available,
        public array $operations,
        public string $sampledAt,
        public ?string $reason,
    ) {}

    /**
     * @param  array<string, mixed>  $data
     */
    public static function fromArray(array $data): self
    {
        $extra = array_diff(array_keys($data), [
            'protocol_version', 'mode', 'available', 'operations', 'sampled_at', 'reason',
        ]);

        if ($extra !== []) {
            throw AgentContractException::invalidParameters(
                'Capability snapshot has unexpected keys: '.implode(', ', $extra).'.'
            );
        }

        $protocolVersion = $data['protocol_version'] ?? null;

        if (! is_int($protocolVersion)) {
            throw AgentContractException::invalidParameters('Capability protocol_version must be an integer.');
        }

        $mode = $data['mode'] ?? null;
        $mode = is_string($mode) ? AgentMode::tryFrom($mode) : null;

        if ($mode === null) {
            throw AgentContractException::invalidParameters('Capability mode is unknown.');
        }

        $available = $data['available'] ?? null;

        if (! is_bool($available)) {
            throw AgentContractException::invalidParameters('Capability available must be a boolean.');
        }

        $operations = $data['operations'] ?? null;

        if (! is_array($operations)) {
            throw AgentContractException::invalidParameters('Capability operations must be an array.');
        }

        foreach ($operations as $operation) {
            if (! is_string($operation) || $operation === '') {
                throw AgentContractException::invalidParameters('Capability operations must be non-empty strings.');
            }
        }

        $sampledAt = $data['sampled_at'] ?? null;

        if (! is_string($sampledAt) || $sampledAt === '') {
            throw AgentContractException::invalidParameters('Capability sampled_at must be a non-empty string.');
        }

        $reason = $data['reason'] ?? null;

        if ($reason !== null && ! is_string($reason)) {
            throw AgentContractException::invalidParameters('Capability reason must be a string or null.');
        }

        return new self($protocolVersion, $mode, $available, array_values($operations), $sampledAt, $reason);
    }

    /**
     * @return array<string, mixed>
     */
    public function toArray(): array
    {
        return [
            'protocol_version' => $this->protocolVersion,
            'mode' => $this->mode->value,
            'available' => $this->available,
            'operations' => $this->operations,
            'sampled_at' => $this->sampledAt,
            'reason' => $this->reason,
        ];
    }
}
