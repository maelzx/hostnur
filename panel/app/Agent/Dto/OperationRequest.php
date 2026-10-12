<?php

namespace App\Agent\Dto;

final readonly class OperationRequest
{
    /**
     * @param  array<string, mixed>  $parameters
     */
    public function __construct(
        public int $protocolVersion,
        public string $requestId,
        public string $idempotencyKey,
        public string $operation,
        public ?ResourceRef $resource,
        public array $parameters,
        public ?int $timeoutSeconds,
    ) {}

    /**
     * @return array<string, mixed>
     */
    public function toArray(): array
    {
        $payload = [
            'protocol_version' => $this->protocolVersion,
            'request_id' => $this->requestId,
            'idempotency_key' => $this->idempotencyKey,
            'operation' => $this->operation,
            'parameters' => $this->parameters === [] ? (object) [] : $this->parameters,
        ];

        if ($this->resource !== null) {
            $payload['resource'] = $this->resource->toArray();
        }

        if ($this->timeoutSeconds !== null) {
            $payload['timeout_seconds'] = $this->timeoutSeconds;
        }

        return $payload;
    }
}
