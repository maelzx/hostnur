<?php

namespace App\Agent\Dto;

use App\Agent\AgentContractException;
use App\Enums\CancellationStatus;
use Illuminate\Support\Str;

final readonly class CancellationReceipt
{
    public function __construct(
        public int $protocolVersion,
        public string $executionId,
        public CancellationStatus $status,
        public string $requestedAt,
    ) {}

    /**
     * @param  array<string, mixed>  $data
     */
    public static function fromArray(array $data): self
    {
        $extra = array_diff(array_keys($data), [
            'protocol_version', 'execution_id', 'status', 'requested_at',
        ]);

        if ($extra !== []) {
            throw AgentContractException::invalidParameters(
                'Cancellation receipt has unexpected keys: '.implode(', ', $extra).'.'
            );
        }

        $protocolVersion = $data['protocol_version'] ?? null;

        if (! is_int($protocolVersion)) {
            throw AgentContractException::invalidParameters('Cancellation receipt protocol_version must be an integer.');
        }

        $executionId = $data['execution_id'] ?? null;

        if (! is_string($executionId) || ! Str::isUuid($executionId)) {
            throw AgentContractException::invalidParameters('Cancellation receipt execution_id must be a UUID.');
        }

        $status = $data['status'] ?? null;
        $status = is_string($status) ? CancellationStatus::tryFrom($status) : null;

        if ($status === null) {
            throw AgentContractException::invalidParameters('Cancellation receipt status is unknown.');
        }

        $requestedAt = $data['requested_at'] ?? null;

        if (! is_string($requestedAt) || $requestedAt === '') {
            throw AgentContractException::invalidParameters('Cancellation receipt requested_at must be a non-empty string.');
        }

        return new self($protocolVersion, $executionId, $status, $requestedAt);
    }

    /**
     * @return array<string, mixed>
     */
    public function toArray(): array
    {
        return [
            'protocol_version' => $this->protocolVersion,
            'execution_id' => $this->executionId,
            'status' => $this->status->value,
            'requested_at' => $this->requestedAt,
        ];
    }
}
