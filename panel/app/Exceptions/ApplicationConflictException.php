<?php

namespace App\Exceptions;

use App\Enums\ErrorCode;
use RuntimeException;

class ApplicationConflictException extends RuntimeException
{
    public function errorCode(): ErrorCode
    {
        return ErrorCode::Conflict;
    }
}
