<?php

namespace Tests\Support;

use PHPUnit\Framework\Assert;

final class ContractFixtures
{
    public static function path(string $relative = ''): string
    {
        return rtrim((string) config('hostnur.contracts_path'), '/').'/'.ltrim($relative, '/');
    }

    /**
     * @return array<string, mixed>
     */
    public static function json(string $relative): array
    {
        $decoded = json_decode((string) file_get_contents(self::path($relative)), true, flags: JSON_THROW_ON_ERROR);

        Assert::assertIsArray($decoded, "Fixture {$relative} is not an object.");

        return $decoded;
    }

    /**
     * @return array<string, array<string, mixed>>
     */
    public static function validFixtures(): array
    {
        return self::glob('fixtures/valid/*.json');
    }

    /**
     * @return array<string, array<string, mixed>>
     */
    public static function fakeFixtures(): array
    {
        return self::glob('fixtures/fake/*.json');
    }

    /**
     * @return array<string, array<string, mixed>>
     */
    public static function validRequests(): array
    {
        return self::glob('fixtures/valid/request-*.json');
    }

    /**
     * @return array<string, array{payload: array<string, mixed>, error: string}>
     */
    public static function invalidRequests(): array
    {
        $manifest = self::json('fixtures/invalid/manifest.json');
        $fixtures = [];

        foreach ($manifest['requests'] as $file => $error) {
            $fixtures[$file] = [
                'payload' => self::json('fixtures/invalid/'.$file),
                'error' => $error,
            ];
        }

        return $fixtures;
    }

    /**
     * @return array<string, array<string, mixed>>
     */
    public static function invalidResponses(): array
    {
        $manifest = self::json('fixtures/invalid/manifest.json');
        $fixtures = [];

        foreach ($manifest['responses'] as $file => $error) {
            $fixtures[$file] = [
                'payload' => self::json('fixtures/invalid/'.$file),
                'error' => $error,
            ];
        }

        return $fixtures;
    }

    /**
     * @return array<string, mixed>
     */
    public static function schema(string $fixtureFile): array
    {
        return self::json(self::schemaPath($fixtureFile));
    }

    public static function schemaPath(string $fixtureFile): string
    {
        return match (true) {
            str_starts_with($fixtureFile, 'request-') => 'schemas/operation-request.schema.json',
            str_starts_with($fixtureFile, 'application-log') => 'schemas/application-log.schema.json',
            str_starts_with($fixtureFile, 'application-metrics') => 'schemas/application-metrics.schema.json',
            str_starts_with($fixtureFile, 'application-observation') => 'schemas/application-observation.schema.json',
            str_starts_with($fixtureFile, 'server-snapshot') => 'schemas/server-snapshot.schema.json',
            str_starts_with($fixtureFile, 'snapshot') => 'schemas/operation-snapshot.schema.json',
            str_starts_with($fixtureFile, 'receipt') => 'schemas/operation-receipt.schema.json',
            str_starts_with($fixtureFile, 'capability') => 'schemas/capability-snapshot.schema.json',
            str_starts_with($fixtureFile, 'cancellation') => 'schemas/cancellation-receipt.schema.json',
            default => Assert::fail("No schema mapping for fixture {$fixtureFile}."),
        };
    }

    /**
     * @return array<string, array<string, mixed>>
     */
    private static function glob(string $pattern): array
    {
        $fixtures = [];

        foreach (glob(self::path($pattern)) as $file) {
            $decoded = json_decode((string) file_get_contents($file), true, flags: JSON_THROW_ON_ERROR);

            Assert::assertIsArray($decoded, "Fixture {$file} is not an object.");

            $fixtures[basename($file)] = $decoded;
        }

        return $fixtures;
    }
}
