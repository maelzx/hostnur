<?php

namespace App\Agent\Dto;

use App\Agent\AgentContractException;
use App\Enums\OperationStatus;
use Illuminate\Support\Str;

final readonly class OperationSnapshot
{
    /**
     * @param  list<array{sequence: int, at: string, message: string}>  $events
     * @param  array<string, mixed>|null  $result
     */
    public function __construct(
        public int $protocolVersion,
        public string $executionId,
        public string $requestId,
        public string $operation,
        public OperationStatus $status,
        public ?int $progress,
        public array $events,
        public ?array $result,
        public ?AgentError $error,
        public string $updatedAt,
    ) {}

    /**
     * @param  array<string, mixed>  $data
     */
    public static function fromArray(array $data): self
    {
        $extra = array_diff(array_keys($data), [
            'protocol_version', 'execution_id', 'request_id', 'operation', 'status',
            'progress', 'events', 'result', 'error', 'updated_at',
        ]);

        if ($extra !== []) {
            throw AgentContractException::invalidParameters(
                'Snapshot has unexpected keys: '.implode(', ', $extra).'.'
            );
        }

        $protocolVersion = $data['protocol_version'] ?? null;

        if (! is_int($protocolVersion)) {
            throw AgentContractException::invalidParameters('Snapshot protocol_version must be an integer.');
        }

        $executionId = self::uuid($data['execution_id'] ?? null, 'execution_id');
        $requestId = self::uuid($data['request_id'] ?? null, 'request_id');

        $operation = $data['operation'] ?? null;

        if (! is_string($operation) || $operation === '') {
            throw AgentContractException::invalidParameters('Snapshot operation must be a non-empty string.');
        }

        $status = $data['status'] ?? null;
        $status = is_string($status) ? OperationStatus::tryFrom($status) : null;

        if ($status === null) {
            throw AgentContractException::invalidParameters('Snapshot status is unknown.');
        }

        $progress = $data['progress'] ?? null;

        if ($progress !== null && (! is_int($progress) || $progress < 0 || $progress > 100)) {
            throw AgentContractException::invalidParameters('Snapshot progress must be an integer between 0 and 100.');
        }

        $events = self::events($data['events'] ?? []);

        $result = $data['result'] ?? null;

        if ($result !== null && ! is_array($result)) {
            throw AgentContractException::invalidParameters('Snapshot result must be an object or null.');
        }

        $error = $data['error'] ?? null;

        if ($error !== null) {
            if (! is_array($error)) {
                throw AgentContractException::invalidParameters('Snapshot error must be an object or null.');
            }

            $error = AgentError::fromArray($error);
        }

        $updatedAt = $data['updated_at'] ?? null;

        if (! is_string($updatedAt) || $updatedAt === '') {
            throw AgentContractException::invalidParameters('Snapshot updated_at must be a non-empty string.');
        }

        return new self(
            $protocolVersion, $executionId, $requestId, $operation, $status,
            $progress, $events, $result, $error, $updatedAt,
        );
    }

    /**
     * @return array<string, mixed>
     */
    public function toArray(): array
    {
        return [
            'protocol_version' => $this->protocolVersion,
            'execution_id' => $this->executionId,
            'request_id' => $this->requestId,
            'operation' => $this->operation,
            'status' => $this->status->value,
            'progress' => $this->progress,
            'events' => $this->events,
            'result' => $this->result,
            'error' => $this->error?->toArray(),
            'updated_at' => $this->updatedAt,
        ];
    }

    /**
     * @return list<array{sequence: int, at: string, message: string}>
     */
    private static function events(mixed $events): array
    {
        if (! is_array($events)) {
            throw AgentContractException::invalidParameters('Snapshot events must be an array.');
        }

        $max = (int) config('hostnur.limits.max_events', 100);

        if (count($events) > $max) {
            throw AgentContractException::invalidParameters("Snapshot may contain at most {$max} events.");
        }

        return array_values(array_map(function (mixed $event): array {
            if (! is_array($event)) {
                throw AgentContractException::invalidParameters('Each snapshot event must be an object.');
            }

            $extra = array_diff(array_keys($event), ['sequence', 'at', 'message']);

            if ($extra !== []) {
                throw AgentContractException::invalidParameters(
                    'Event has unexpected keys: '.implode(', ', $extra).'.'
                );
            }

            $sequence = $event['sequence'] ?? null;
            $at = $event['at'] ?? null;
            $message = $event['message'] ?? null;

            if (! is_int($sequence) || $sequence < 0) {
                throw AgentContractException::invalidParameters('Event sequence must be a non-negative integer.');
            }

            if (! is_string($at) || $at === '' || ! is_string($message)) {
                throw AgentContractException::invalidParameters('Event at/message must be strings.');
            }

            return ['sequence' => $sequence, 'at' => $at, 'message' => $message];
        }, $events));
    }

    private static function uuid(mixed $value, string $field): string
    {
        if (! is_string($value) || ! Str::isUuid($value)) {
            throw AgentContractException::invalidParameters("Snapshot {$field} must be a UUID.");
        }

        return $value;
    }
}
