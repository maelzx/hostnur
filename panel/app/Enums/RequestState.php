<?php

namespace App\Enums;

enum RequestState: string
{
    case Requested = 'requested';
    case Running = 'running';
    case Succeeded = 'succeeded';
    case Failed = 'failed';
    case Cancelled = 'cancelled';
    case Unknown = 'unknown';

    public static function fromTaskStatus(TaskStatus $status): self
    {
        return match ($status) {
            TaskStatus::Pending, TaskStatus::AwaitingApproval, TaskStatus::Queued => self::Requested,
            TaskStatus::Running => self::Running,
            TaskStatus::Succeeded => self::Succeeded,
            TaskStatus::Failed => self::Failed,
            TaskStatus::Cancelled => self::Cancelled,
            TaskStatus::Unknown => self::Unknown,
        };
    }

    public function isTerminal(): bool
    {
        return in_array($this, [self::Succeeded, self::Failed, self::Cancelled], true);
    }

    public function label(): string
    {
        return str_replace('_', ' ', $this->value);
    }
}
