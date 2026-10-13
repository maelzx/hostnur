<?php

namespace App\Agent;

use App\Agent\Dto\AgentError;
use App\Agent\Dto\OperationReceipt;
use App\Agent\Dto\OperationSnapshot;
use App\Enums\ErrorCode;
use App\Enums\FakeScenario;
use App\Enums\OperationStatus;
use App\Models\FakeAgentExecution;
use Illuminate\Support\Carbon;

/**
 * Derives a fake execution's observable state from its persisted record, its
 * scenario and the current clock. There is no worker and no in-memory state, so a
 * later process sees the same deterministic progression.
 */
final class FakeExecutionSimulator
{
    public function receipt(FakeAgentExecution $record): OperationReceipt
    {
        return new OperationReceipt(
            protocolVersion: $this->protocolVersion(),
            requestId: $record->request_id,
            executionId: $record->execution_id,
            operation: $record->operation,
            status: $this->status($record),
            acceptedAt: $record->created_at->toIso8601String(),
        );
    }

    public function snapshot(FakeAgentExecution $record): OperationSnapshot
    {
        $derived = $this->derive($record);

        return new OperationSnapshot(
            protocolVersion: $this->protocolVersion(),
            executionId: $record->execution_id,
            requestId: $record->request_id,
            operation: $record->operation,
            status: $derived['status'],
            progress: $derived['progress'],
            events: $derived['events'],
            result: $derived['result'],
            error: $derived['error'],
            updatedAt: $derived['updated_at'],
        );
    }

    public function status(FakeAgentExecution $record): OperationStatus
    {
        return $this->derive($record)['status'];
    }

    /**
     * @return array{
     *     status: OperationStatus,
     *     progress: int|null,
     *     events: list<array{sequence: int, at: string, message: string}>,
     *     result: array<string, mixed>|null,
     *     error: AgentError|null,
     *     updated_at: string
     * }
     */
    private function derive(FakeAgentExecution $record): array
    {
        $created = $record->created_at;
        $label = "Simulated {$record->operation}";
        $duration = max(1, (int) config('hostnur.fake.duration_seconds', 2));
        $timeout = max(1, (int) config('hostnur.fake.timeout_seconds', 3));

        if ($record->cancel_requested_at !== null) {
            $cancelledAt = $record->cancel_requested_at;

            return [
                'status' => OperationStatus::Cancelled,
                'progress' => $this->runningProgress($created, $cancelledAt, $duration),
                'events' => [
                    $this->event(0, $created, "{$label} accepted."),
                    $this->event(1, $cancelledAt, "{$label} cancellation requested."),
                    $this->event(2, $cancelledAt, "{$label} cancelled."),
                ],
                'result' => null,
                'error' => null,
                'updated_at' => $cancelledAt->toIso8601String(),
            ];
        }

        $scenario = FakeScenario::tryFrom($record->scenario) ?? FakeScenario::Success;
        $elapsed = $this->elapsedSeconds($created);

        if ($scenario === FakeScenario::Timeout) {
            if ($elapsed >= $timeout) {
                return $this->terminal(
                    OperationStatus::TimedOut,
                    $created,
                    $created->copy()->addSeconds($timeout),
                    new AgentError(ErrorCode::Timeout, "{$label} timed out.", retryable: true),
                    $label,
                );
            }

            return $this->runningOrAccepted(
                $created,
                $elapsed,
                $timeout,
                $this->runningProgress($created, now(), $timeout),
                $label,
            );
        }

        if ($elapsed >= $duration) {
            if ($scenario === FakeScenario::Failure) {
                return $this->terminal(
                    OperationStatus::Failed,
                    $created,
                    $created->copy()->addSeconds($duration),
                    new AgentError(ErrorCode::OperationFailed, "{$label} failed.", retryable: false),
                    $label,
                );
            }

            return $this->terminal(
                OperationStatus::Succeeded,
                $created,
                $created->copy()->addSeconds($duration),
                null,
                $label,
                ['simulated' => true, 'operation' => $record->operation],
            );
        }

        return $this->runningOrAccepted(
            $created,
            $elapsed,
            $duration,
            $this->runningProgress($created, now(), $duration),
            $label,
        );
    }

    /**
     * @return array{status: OperationStatus, progress: int|null, events: list<array{sequence: int, at: string, message: string}>, result: array<string, mixed>|null, error: AgentError|null, updated_at: string}
     */
    private function runningOrAccepted(Carbon $created, int $elapsed, int $duration, int $progress, string $label): array
    {
        if ($elapsed <= 0) {
            return [
                'status' => OperationStatus::Accepted,
                'progress' => 0,
                'events' => [$this->event(0, $created, "{$label} accepted.")],
                'result' => null,
                'error' => null,
                'updated_at' => $created->toIso8601String(),
            ];
        }

        return [
            'status' => OperationStatus::Running,
            'progress' => $progress,
            'events' => [
                $this->event(0, $created, "{$label} accepted."),
                $this->event(1, $created->copy()->addSecond(), "{$label} running."),
            ],
            'result' => null,
            'error' => null,
            'updated_at' => now()->toIso8601String(),
        ];
    }

    /**
     * @param  array<string, mixed>|null  $result
     * @return array{status: OperationStatus, progress: int|null, events: list<array{sequence: int, at: string, message: string}>, result: array<string, mixed>|null, error: AgentError|null, updated_at: string}
     */
    private function terminal(
        OperationStatus $status,
        Carbon $created,
        Carbon $finishedAt,
        ?AgentError $error,
        string $label,
        ?array $result = null,
    ): array {
        $message = match ($status) {
            OperationStatus::Succeeded => "{$label} succeeded.",
            OperationStatus::TimedOut => "{$label} timed out.",
            default => "{$label} failed.",
        };

        return [
            'status' => $status,
            'progress' => 100,
            'events' => [
                $this->event(0, $created, "{$label} accepted."),
                $this->event(1, $created->copy()->addSecond(), "{$label} running."),
                $this->event(2, $finishedAt, $message),
            ],
            'result' => $result,
            'error' => $error,
            'updated_at' => $finishedAt->toIso8601String(),
        ];
    }

    /**
     * @return array{sequence: int, at: string, message: string}
     */
    private function event(int $sequence, Carbon $at, string $message): array
    {
        return ['sequence' => $sequence, 'at' => $at->toIso8601String(), 'message' => $message];
    }

    private function elapsedSeconds(Carbon $created): int
    {
        $now = now();

        return $now->greaterThan($created) ? (int) $created->diffInSeconds($now) : 0;
    }

    private function runningProgress(Carbon $created, Carbon $at, int $duration): int
    {
        $elapsed = $at->greaterThan($created) ? (int) $created->diffInSeconds($at) : 0;

        return min(99, (int) floor($elapsed / $duration * 100));
    }

    private function protocolVersion(): int
    {
        return (int) config('hostnur.protocol_version', 1);
    }
}
