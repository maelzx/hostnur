<?php

namespace App\Agent\Exceptions;

use App\Enums\ErrorCode;
use RuntimeException;

class AgentUnavailableException extends RuntimeException
{
    public function errorCode(): ErrorCode
    {
        return ErrorCode::AgentUnavailable;
    }
}
