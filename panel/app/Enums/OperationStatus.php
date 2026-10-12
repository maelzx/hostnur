<?php

namespace App\Enums;

enum OperationStatus: string
{
    case Accepted = 'accepted';
    case Running = 'running';
    case Succeeded = 'succeeded';
    case Failed = 'failed';
    case Cancelled = 'cancelled';
    case TimedOut = 'timed_out';

    /**
     * A status whose outcome can still change without another submission.
     */
    public function isActive(): bool
    {
        return $this === self::Accepted || $this === self::Running;
    }

    public function isTerminal(): bool
    {
        return ! $this->isActive();
    }
}
