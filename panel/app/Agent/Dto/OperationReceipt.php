<?php

namespace App\Agent\Dto;

use App\Agent\AgentContractException;
use App\Enums\OperationStatus;
use Illuminate\Support\Str;

final readonly class OperationReceipt
{
    public function __construct(
        public int $protocolVersion,
        public string $requestId,
        public string $executionId,
        public string $operation,
        public OperationStatus $status,
        public string $acceptedAt,
    ) {}

    /**
     * @param  array<string, mixed>  $data
     */
    public static function fromArray(array $data): self
    {
        $extra = array_diff(array_keys($data), [
            'protocol_version', 'request_id', 'execution_id', 'operation', 'status', 'accepted_at',
        ]);

        if ($extra !== []) {
            throw AgentContractException::invalidParameters(
                'Receipt has unexpected keys: '.implode(', ', $extra).'.'
            );
        }

        $protocolVersion = $data['protocol_version'] ?? null;

        if (! is_int($protocolVersion)) {
            throw AgentContractException::invalidParameters('Receipt protocol_version must be an integer.');
        }

        $requestId = self::uuid($data['request_id'] ?? null, 'request_id');
        $executionId = self::uuid($data['execution_id'] ?? null, 'execution_id');

        $operation = $data['operation'] ?? null;

        if (! is_string($operation) || $operation === '') {
            throw AgentContractException::invalidParameters('Receipt operation must be a non-empty string.');
        }

        $status = $data['status'] ?? null;
        $status = is_string($status) ? OperationStatus::tryFrom($status) : null;

        if ($status === null) {
            throw AgentContractException::invalidParameters('Receipt status is unknown.');
        }

        $acceptedAt = $data['accepted_at'] ?? null;

        if (! is_string($acceptedAt) || $acceptedAt === '') {
            throw AgentContractException::invalidParameters('Receipt accepted_at must be a non-empty string.');
        }

        return new self($protocolVersion, $requestId, $executionId, $operation, $status, $acceptedAt);
    }

    /**
     * @return array<string, mixed>
     */
    public function toArray(): array
    {
        return [
            'protocol_version' => $this->protocolVersion,
            'request_id' => $this->requestId,
            'execution_id' => $this->executionId,
            'operation' => $this->operation,
            'status' => $this->status->value,
            'accepted_at' => $this->acceptedAt,
        ];
    }

    private static function uuid(mixed $value, string $field): string
    {
        if (! is_string($value) || ! Str::isUuid($value)) {
            throw AgentContractException::invalidParameters("Receipt {$field} must be a UUID.");
        }

        return $value;
    }
}
