<?php

namespace App\Enums;

enum TaskStatus: string
{
    case Pending = 'pending';
    case AwaitingApproval = 'awaiting_approval';
    case Queued = 'queued';
    case Running = 'running';
    case Succeeded = 'succeeded';
    case Failed = 'failed';
    case Cancelled = 'cancelled';
    case Unknown = 'unknown';

    /**
     * Terminal states cannot be advanced by a late poll. `Unknown` is a
     * provisional state that only a deliberate reconciliation moves.
     */
    public function isTerminal(): bool
    {
        return in_array($this, [self::Succeeded, self::Failed, self::Cancelled], true);
    }

    public function isActive(): bool
    {
        return in_array($this, [self::Pending, self::AwaitingApproval, self::Queued, self::Running], true);
    }

    /**
     * @return list<self>
     */
    public function allowedTransitions(): array
    {
        return match ($this) {
            self::Pending => [self::AwaitingApproval, self::Queued, self::Cancelled],
            self::AwaitingApproval => [self::Queued, self::Cancelled, self::Failed],
            self::Queued => [self::Running, self::AwaitingApproval, self::Cancelled, self::Failed, self::Unknown],
            self::Running => [self::Succeeded, self::Failed, self::Cancelled, self::Unknown],
            self::Unknown => [self::Succeeded, self::Failed, self::Cancelled],
            self::Succeeded, self::Failed, self::Cancelled => [],
        };
    }

    public function canTransitionTo(self $status): bool
    {
        return in_array($status, $this->allowedTransitions(), true);
    }

    public static function fromOperation(OperationStatus $status): self
    {
        return match ($status) {
            OperationStatus::Accepted, OperationStatus::Running => self::Running,
            OperationStatus::Succeeded => self::Succeeded,
            OperationStatus::Failed, OperationStatus::TimedOut => self::Failed,
            OperationStatus::Cancelled => self::Cancelled,
        };
    }

    public function label(): string
    {
        return str_replace('_', ' ', $this->value);
    }
}
