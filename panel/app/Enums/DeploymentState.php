<?php

namespace App\Enums;

enum DeploymentState: string
{
    case Requested = 'requested';
    case Running = 'running';
    case Succeeded = 'succeeded';
    case Failed = 'failed';
    case Cancelled = 'cancelled';
    case Unknown = 'unknown';

    public function isTerminal(): bool
    {
        return in_array($this, [self::Succeeded, self::Failed, self::Cancelled], true);
    }

    public function label(): string
    {
        return str_replace('_', ' ', $this->value);
    }
}
