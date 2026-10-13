<?php

namespace App\Exceptions;

use App\Enums\TaskStatus;
use RuntimeException;

class InvalidTaskTransitionException extends RuntimeException
{
    public static function between(TaskStatus $from, TaskStatus $to): self
    {
        return new self("Task cannot transition from {$from->value} to {$to->value}.");
    }
}
