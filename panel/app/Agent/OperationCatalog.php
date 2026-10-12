<?php

namespace App\Agent;

use RuntimeException;

final class OperationCatalog
{
    /**
     * @var array{protocol_versions: list<int>, operations: array<string, array<string, mixed>>}|null
     */
    private ?array $catalog = null;

    /**
     * @return list<int>
     */
    public function protocolVersions(): array
    {
        return array_values($this->load()['protocol_versions']);
    }

    public function supports(string $operation): bool
    {
        return array_key_exists($operation, $this->operations());
    }

    /**
     * @return array<string, mixed>|null
     */
    public function definition(string $operation): ?array
    {
        return $this->operations()[$operation] ?? null;
    }

    /**
     * @return array<string, array<string, mixed>>
     */
    public function operations(): array
    {
        return $this->load()['operations'];
    }

    /**
     * @return list<string>
     */
    public function names(): array
    {
        return array_keys($this->operations());
    }

    /**
     * @return array{protocol_versions: list<int>, operations: array<string, array<string, mixed>>}
     */
    private function load(): array
    {
        if ($this->catalog !== null) {
            return $this->catalog;
        }

        $path = rtrim((string) config('hostnur.contracts_path'), '/').'/catalog.json';

        if (! is_file($path)) {
            throw new RuntimeException("Agent operation catalog was not found at {$path}.");
        }

        $decoded = json_decode((string) file_get_contents($path), true, flags: JSON_THROW_ON_ERROR);

        if (! is_array($decoded)
            || ! is_array($decoded['protocol_versions'] ?? null)
            || ! is_array($decoded['operations'] ?? null)) {
            throw new RuntimeException('Agent operation catalog is malformed.');
        }

        return $this->catalog = $decoded;
    }
}
