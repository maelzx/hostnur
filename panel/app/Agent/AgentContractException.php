<?php

namespace App\Agent;

use App\Enums\ErrorCode;
use RuntimeException;

class AgentContractException extends RuntimeException
{
    public function __construct(
        public readonly ErrorCode $errorCode,
        string $message,
    ) {
        parent::__construct($message);
    }

    public static function unsupportedProtocol(string $message): self
    {
        return new self(ErrorCode::UnsupportedProtocol, $message);
    }

    public static function unsupportedOperation(string $message): self
    {
        return new self(ErrorCode::UnsupportedOperation, $message);
    }

    public static function invalidParameters(string $message): self
    {
        return new self(ErrorCode::InvalidParameters, $message);
    }

    public static function conflict(string $message): self
    {
        return new self(ErrorCode::Conflict, $message);
    }

    public static function executionUnknown(string $message): self
    {
        return new self(ErrorCode::ExecutionUnknown, $message);
    }
}
