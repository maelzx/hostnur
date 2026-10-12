<?php

namespace App\Agent\Dto;

use App\Agent\AgentContractException;
use App\Enums\ErrorCode;

final readonly class AgentError
{
    public function __construct(
        public ErrorCode $code,
        public string $message,
        public bool $retryable = false,
    ) {}

    /**
     * @param  array<string, mixed>  $data
     */
    public static function fromArray(array $data): self
    {
        $extra = array_diff(array_keys($data), ['code', 'message', 'retryable']);

        if ($extra !== []) {
            throw AgentContractException::invalidParameters(
                'Agent error has unexpected keys: '.implode(', ', $extra).'.'
            );
        }

        $code = $data['code'] ?? null;
        $code = is_string($code) ? ErrorCode::tryFrom($code) : null;

        if ($code === null) {
            throw AgentContractException::invalidParameters('Agent error code is unknown.');
        }

        $message = $data['message'] ?? null;

        if (! is_string($message) || trim($message) === '') {
            throw AgentContractException::invalidParameters('Agent error message must be a non-empty string.');
        }

        $retryable = $data['retryable'] ?? false;

        if (! is_bool($retryable)) {
            throw AgentContractException::invalidParameters('Agent error retryable must be a boolean.');
        }

        return new self($code, $message, $retryable);
    }

    /**
     * @return array{code: string, message: string, retryable: bool}
     */
    public function toArray(): array
    {
        return [
            'code' => $this->code->value,
            'message' => $this->message,
            'retryable' => $this->retryable,
        ];
    }
}
