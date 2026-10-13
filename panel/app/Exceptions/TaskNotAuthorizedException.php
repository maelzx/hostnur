<?php

namespace App\Exceptions;

use App\Enums\ErrorCode;
use RuntimeException;

class TaskNotAuthorizedException extends RuntimeException
{
    public function errorCode(): ErrorCode
    {
        return ErrorCode::PermissionDenied;
    }
}
