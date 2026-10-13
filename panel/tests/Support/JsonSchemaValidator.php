<?php

namespace Tests\Support;

use PHPUnit\Framework\Assert;

/**
 * Minimal JSON Schema (2020-12 subset) checker for checked-in contract fixtures.
 * It supports only the keywords used by contracts/v1/schemas and is test-only, so
 * no schema-validation dependency is required.
 */
final class JsonSchemaValidator
{
    /**
     * @param  array<string, mixed>  $schema
     */
    public static function assertValid(array $schema, mixed $data, string $path = '$'): void
    {
        $error = self::error($schema, $data, $path);

        Assert::assertNull($error, $error ?? '');
    }

    /**
     * @param  array<string, mixed>  $schema
     */
    public static function error(array $schema, mixed $data, string $path): ?string
    {
        if (array_key_exists('const', $schema) && $data !== $schema['const']) {
            return "{$path}: expected const ".json_encode($schema['const']);
        }

        if (array_key_exists('enum', $schema) && ! in_array($data, $schema['enum'], true)) {
            return "{$path}: value is not an allowed enum value";
        }

        if (isset($schema['oneOf'])) {
            $matches = 0;

            foreach ($schema['oneOf'] as $subSchema) {
                if (self::error($subSchema, $data, $path) === null) {
                    $matches++;
                }
            }

            return $matches === 1 ? null : "{$path}: expected exactly one oneOf match, got {$matches}";
        }

        if (isset($schema['type']) && ! self::matchesAnyType((array) $schema['type'], $data)) {
            return "{$path}: expected type ".implode('|', (array) $schema['type']);
        }

        if (is_string($data)) {
            if (isset($schema['minLength']) && mb_strlen($data) < $schema['minLength']) {
                return "{$path}: shorter than minLength";
            }

            if (isset($schema['maxLength']) && mb_strlen($data) > $schema['maxLength']) {
                return "{$path}: longer than maxLength";
            }

            if (($schema['format'] ?? null) === 'uuid' && ! self::isUuid($data)) {
                return "{$path}: not a UUID";
            }
        }

        if ((is_int($data) || is_float($data)) && ! is_bool($data)) {
            if (isset($schema['minimum']) && $data < $schema['minimum']) {
                return "{$path}: below minimum";
            }

            if (isset($schema['maximum']) && $data > $schema['maximum']) {
                return "{$path}: above maximum";
            }
        }

        if (self::isObject($data)) {
            foreach (($schema['required'] ?? []) as $key) {
                if (! array_key_exists($key, $data)) {
                    return "{$path}: missing required key {$key}";
                }
            }

            $properties = $schema['properties'] ?? [];

            if (($schema['additionalProperties'] ?? true) === false) {
                foreach (array_keys($data) as $key) {
                    if (! array_key_exists($key, $properties)) {
                        return "{$path}: unexpected key {$key}";
                    }
                }
            }

            foreach ($properties as $key => $subSchema) {
                if (array_key_exists($key, $data)) {
                    $error = self::error($subSchema, $data[$key], "{$path}.{$key}");

                    if ($error !== null) {
                        return $error;
                    }
                }
            }
        }

        if (is_array($data) && array_is_list($data)) {
            if (isset($schema['minItems']) && count($data) < $schema['minItems']) {
                return "{$path}: too few items";
            }

            if (isset($schema['maxItems']) && count($data) > $schema['maxItems']) {
                return "{$path}: too many items";
            }

            if (isset($schema['items'])) {
                foreach ($data as $index => $item) {
                    $error = self::error($schema['items'], $item, "{$path}[{$index}]");

                    if ($error !== null) {
                        return $error;
                    }
                }
            }
        }

        return null;
    }

    /**
     * @param  list<string>  $types
     */
    private static function matchesAnyType(array $types, mixed $data): bool
    {
        foreach ($types as $type) {
            if (self::matchesType($type, $data)) {
                return true;
            }
        }

        return false;
    }

    private static function matchesType(string $type, mixed $data): bool
    {
        return match ($type) {
            'object' => self::isObject($data),
            'array' => is_array($data) && array_is_list($data),
            'string' => is_string($data),
            'integer' => is_int($data),
            'number' => is_int($data) || is_float($data),
            'boolean' => is_bool($data),
            'null' => $data === null,
            default => false,
        };
    }

    private static function isObject(mixed $data): bool
    {
        return is_array($data) && ($data === [] || ! array_is_list($data));
    }

    private static function isUuid(string $value): bool
    {
        return (bool) preg_match(
            '/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i',
            $value
        );
    }
}
