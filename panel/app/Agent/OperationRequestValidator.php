<?php

namespace App\Agent;

use App\Agent\Dto\OperationRequest;
use App\Agent\Dto\ResourceRef;
use Illuminate\Support\Facades\Validator;
use Illuminate\Support\Str;

final class OperationRequestValidator
{
    /**
     * @var list<string>
     */
    private const ENVELOPE_KEYS = [
        'protocol_version',
        'request_id',
        'idempotency_key',
        'operation',
        'resource',
        'parameters',
        'timeout_seconds',
    ];

    public function __construct(
        private readonly OperationCatalog $catalog,
    ) {}

    /**
     * @param  array<string, mixed>  $payload
     */
    public function validate(array $payload): OperationRequest
    {
        $this->assertWithinMessageLimit($payload);

        $extra = array_diff(array_keys($payload), self::ENVELOPE_KEYS);

        if ($extra !== []) {
            throw AgentContractException::invalidParameters(
                'Request has unexpected keys: '.implode(', ', $extra).'.'
            );
        }

        $validator = Validator::make($payload, [
            'protocol_version' => ['required', 'integer'],
            'request_id' => ['required', 'string', 'uuid'],
            'idempotency_key' => ['required', 'string', 'uuid'],
            'operation' => ['required', 'string'],
            'resource' => ['nullable', 'array'],
            'parameters' => ['sometimes', 'array'],
            'timeout_seconds' => ['sometimes', 'nullable', 'integer', 'min:1', 'max:86400'],
        ]);

        if ($validator->fails()) {
            throw AgentContractException::invalidParameters(
                implode(' ', $validator->errors()->all())
            );
        }

        if (! in_array($payload['protocol_version'], $this->catalog->protocolVersions(), true)) {
            throw AgentContractException::unsupportedProtocol(
                "Protocol version {$payload['protocol_version']} is not supported."
            );
        }

        $operation = $payload['operation'];
        $definition = $this->catalog->definition($operation);

        if ($definition === null) {
            throw AgentContractException::unsupportedOperation(
                "Operation {$operation} is not in the catalog."
            );
        }

        $resource = $this->validateResource($definition, $payload);
        $parameters = $this->validateParameters($definition, $payload['parameters'] ?? []);

        return new OperationRequest(
            $payload['protocol_version'],
            $payload['request_id'],
            $payload['idempotency_key'],
            $operation,
            $resource,
            $parameters,
            $payload['timeout_seconds'] ?? null,
        );
    }

    /**
     * @param  array<string, mixed>  $payload
     */
    private function assertWithinMessageLimit(array $payload): void
    {
        $max = (int) config('hostnur.limits.max_message_bytes', 1_048_576);
        $encoded = json_encode($payload);

        if ($encoded !== false && strlen($encoded) > $max) {
            throw AgentContractException::invalidParameters(
                "Request exceeds the maximum message size of {$max} bytes."
            );
        }
    }

    /**
     * @param  array<string, mixed>  $definition
     * @param  array<string, mixed>  $payload
     */
    private function validateResource(array $definition, array $payload): ?ResourceRef
    {
        $expected = $definition['resource'] ?? null;
        $provided = $payload['resource'] ?? null;

        if ($expected === null) {
            if (array_key_exists('resource', $payload) && $provided !== null) {
                throw AgentContractException::invalidParameters('This operation does not accept a resource reference.');
            }

            return null;
        }

        if (! is_array($provided)) {
            throw AgentContractException::invalidParameters('This operation requires a resource reference.');
        }

        $resource = ResourceRef::fromArray($provided);

        if ($resource->kind->value !== $expected) {
            throw AgentContractException::invalidParameters(
                "Resource kind {$resource->kind->value} is not valid for this operation."
            );
        }

        return $resource;
    }

    /**
     * @param  array<string, mixed>  $definition
     * @return array<string, mixed>
     */
    private function validateParameters(array $definition, mixed $parameters): array
    {
        if (! is_array($parameters)) {
            throw AgentContractException::invalidParameters('Parameters must be an object.');
        }

        if ($parameters !== [] && array_is_list($parameters)) {
            throw AgentContractException::invalidParameters('Parameters must be an object with named keys.');
        }

        $max = (int) config('hostnur.limits.max_parameters', 64);

        if (count($parameters) > $max) {
            throw AgentContractException::invalidParameters("At most {$max} parameters are allowed.");
        }

        $schema = $definition['parameters'] ?? [];
        $extra = array_diff(array_keys($parameters), array_keys($schema));

        if ($extra !== []) {
            throw AgentContractException::invalidParameters(
                'Unknown parameters: '.implode(', ', $extra).'.'
            );
        }

        $validated = [];

        foreach ($schema as $name => $rule) {
            if (! array_key_exists($name, $parameters)) {
                if (($rule['required'] ?? false) === true) {
                    throw AgentContractException::invalidParameters("Parameter {$name} is required.");
                }

                continue;
            }

            $this->assertParameterType($name, $parameters[$name], $rule);
            $validated[$name] = $parameters[$name];
        }

        return $validated;
    }

    /**
     * @param  array<string, mixed>  $rule
     */
    private function assertParameterType(string $name, mixed $value, array $rule): void
    {
        $type = $rule['type'] ?? 'string';

        $valid = match ($type) {
            'uuid' => is_string($value) && Str::isUuid($value),
            'string' => is_string($value) && $value !== '' && strlen($value) <= (int) ($rule['max'] ?? 255),
            'integer' => is_int($value),
            'boolean' => is_bool($value),
            'enum' => is_string($value) && in_array($value, $rule['values'] ?? [], true),
            'string_array' => is_array($value)
                && ! array_is_list($value)
                && count($value) <= (int) ($rule['max_items'] ?? 100)
                && collect($value)->every(fn (mixed $item): bool => is_string($item)),
            default => false,
        };

        if (! $valid) {
            throw AgentContractException::invalidParameters("Parameter {$name} has an invalid value.");
        }
    }
}
