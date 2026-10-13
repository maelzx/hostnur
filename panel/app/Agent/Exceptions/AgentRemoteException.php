<?php

namespace App\Agent\Exceptions;

use App\Enums\ErrorCode;
use RuntimeException;

/**
 * The agent returned a structured error for a request.
 */
class AgentRemoteException extends RuntimeException
{
    public function __construct(
        private readonly ErrorCode $errorCode,
        string $message,
        private readonly bool $retryable = false,
    ) {
        parent::__construct($message);
    }

    public function errorCode(): ErrorCode
    {
        return $this->errorCode;
    }

    public function isRetryable(): bool
    {
        return $this->retryable;
    }
}
