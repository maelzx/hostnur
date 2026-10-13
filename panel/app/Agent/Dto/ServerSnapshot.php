<?php

namespace App\Agent\Dto;

use App\Agent\AgentContractException;
use App\Enums\AgentMode;
use Illuminate\Support\Carbon;

final readonly class ServerSnapshot
{
    /**
     * @param  list<array{name: string, value: int|float, unit: string|null}>  $metrics
     * @param  list<array{name: string, status: string}>  $services
     */
    public function __construct(
        public int $protocolVersion,
        public AgentMode $mode,
        public bool $available,
        public string $sampledAt,
        public array $metrics,
        public array $services,
        public ?string $reason,
        public ?string $hostname = null,
        public ?string $os = null,
        public ?array $firewall = null,
    ) {}

    /**
     * @param  array<string, mixed>  $data
     */
    public static function fromArray(array $data): self
    {
        $extra = array_diff(array_keys($data), [
            'protocol_version', 'mode', 'available', 'sampled_at', 'metrics', 'services', 'reason', 'hostname', 'os', 'firewall',
        ]);

        if ($extra !== []) {
            throw AgentContractException::invalidParameters(
                'Server snapshot has unexpected keys: '.implode(', ', $extra).'.'
            );
        }

        $protocolVersion = $data['protocol_version'] ?? null;

        if (! is_int($protocolVersion)) {
            throw AgentContractException::invalidParameters('Server snapshot protocol_version must be an integer.');
        }

        $mode = $data['mode'] ?? null;
        $mode = is_string($mode) ? AgentMode::tryFrom($mode) : null;

        if ($mode === null) {
            throw AgentContractException::invalidParameters('Server snapshot mode is unknown.');
        }

        $available = $data['available'] ?? null;

        if (! is_bool($available)) {
            throw AgentContractException::invalidParameters('Server snapshot available must be a boolean.');
        }

        $sampledAt = $data['sampled_at'] ?? null;

        if (! is_string($sampledAt) || $sampledAt === '') {
            throw AgentContractException::invalidParameters('Server snapshot sampled_at must be a non-empty string.');
        }

        $metrics = self::metrics($data['metrics'] ?? []);
        $services = self::services($data['services'] ?? []);

        $reason = $data['reason'] ?? null;

        if ($reason !== null && ! is_string($reason)) {
            throw AgentContractException::invalidParameters('Server snapshot reason must be a string or null.');
        }

        $hostname = $data['hostname'] ?? null;

        if ($hostname !== null && ! is_string($hostname)) {
            throw AgentContractException::invalidParameters('Server snapshot hostname must be a string or null.');
        }

        $os = $data['os'] ?? null;

        if ($os !== null && ! is_string($os)) {
            throw AgentContractException::invalidParameters('Server snapshot os must be a string or null.');
        }

        $firewall = self::firewall($data['firewall'] ?? null);

        return new self($protocolVersion, $mode, $available, $sampledAt, $metrics, $services, $reason, $hostname, $os, $firewall);
    }

    /**
     * @return array<string, mixed>
     */
    public function toArray(): array
    {
        return [
            'protocol_version' => $this->protocolVersion,
            'mode' => $this->mode->value,
            'available' => $this->available,
            'sampled_at' => $this->sampledAt,
            'metrics' => $this->metrics,
            'services' => $this->services,
            'reason' => $this->reason,
            'hostname' => $this->hostname,
            'os' => $this->os,
            'firewall' => $this->firewall,
        ];
    }

    /**
     * Distinguish a stale observation from a fresh one. Combined with
     * `mode`/`available`, callers can tell fresh fake data, stale data and
     * offline data apart.
     */
    public function isStale(int $maxAgeSeconds): bool
    {
        return Carbon::parse($this->sampledAt)->lt(now()->subSeconds($maxAgeSeconds));
    }

    /**
     * @return list<array{name: string, value: int|float, unit: string|null}>
     */
    private static function metrics(mixed $metrics): array
    {
        if (! is_array($metrics)) {
            throw AgentContractException::invalidParameters('Server snapshot metrics must be an array.');
        }

        self::assertBounded($metrics, 'metrics');

        return array_values(array_map(function (mixed $metric): array {
            if (! is_array($metric)) {
                throw AgentContractException::invalidParameters('Each metric must be an object.');
            }

            $extra = array_diff(array_keys($metric), ['name', 'value', 'unit']);

            if ($extra !== []) {
                throw AgentContractException::invalidParameters(
                    'Metric has unexpected keys: '.implode(', ', $extra).'.'
                );
            }

            $name = $metric['name'] ?? null;
            $value = $metric['value'] ?? null;
            $unit = $metric['unit'] ?? null;

            if (! is_string($name) || $name === '') {
                throw AgentContractException::invalidParameters('Metric name must be a non-empty string.');
            }

            if (! is_int($value) && ! is_float($value)) {
                throw AgentContractException::invalidParameters('Metric value must be numeric.');
            }

            if ($unit !== null && ! is_string($unit)) {
                throw AgentContractException::invalidParameters('Metric unit must be a string or null.');
            }

            return ['name' => $name, 'value' => $value, 'unit' => $unit];
        }, $metrics));
    }

    /**
     * @return list<array{name: string, status: string}>
     */
    private static function services(mixed $services): array
    {
        if (! is_array($services)) {
            throw AgentContractException::invalidParameters('Server snapshot services must be an array.');
        }

        self::assertBounded($services, 'services');

        return array_values(array_map(function (mixed $service): array {
            if (! is_array($service)) {
                throw AgentContractException::invalidParameters('Each service must be an object.');
            }

            $extra = array_diff(array_keys($service), ['name', 'status']);

            if ($extra !== []) {
                throw AgentContractException::invalidParameters(
                    'Service has unexpected keys: '.implode(', ', $extra).'.'
                );
            }

            $name = $service['name'] ?? null;
            $status = $service['status'] ?? null;

            if (! is_string($name) || $name === '' || ! is_string($status) || $status === '') {
                throw AgentContractException::invalidParameters('Service name/status must be non-empty strings.');
            }

            return ['name' => $name, 'status' => $status];
        }, $services));
    }

    private static function assertBounded(array $items, string $label): void
    {
        $max = (int) config('hostnur.limits.max_observations', 100);

        if (count($items) > $max) {
            throw AgentContractException::invalidParameters("Server snapshot may contain at most {$max} {$label}.");
        }
    }

    /**
     * @return array{enabled: bool, incoming_policy: string, rule_count: int}|null
     */
    private static function firewall(mixed $firewall): ?array
    {
        if ($firewall === null) {
            return null;
        }

        if (! is_array($firewall)) {
            throw AgentContractException::invalidParameters('Server snapshot firewall must be an object or null.');
        }

        $extra = array_diff(array_keys($firewall), ['enabled', 'incoming_policy', 'rule_count']);

        if ($extra !== []) {
            throw AgentContractException::invalidParameters(
                'Firewall summary has unexpected keys: '.implode(', ', $extra).'.'
            );
        }

        $enabled = $firewall['enabled'] ?? null;
        $policy = $firewall['incoming_policy'] ?? null;
        $count = $firewall['rule_count'] ?? null;

        if (! is_bool($enabled) || ! is_string($policy) || $policy === '' || ! is_int($count) || $count < 0) {
            throw AgentContractException::invalidParameters('Firewall summary is invalid.');
        }

        return ['enabled' => $enabled, 'incoming_policy' => $policy, 'rule_count' => $count];
    }
}
