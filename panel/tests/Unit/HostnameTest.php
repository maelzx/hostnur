<?php

namespace Tests\Unit;

use App\Support\Hostname;
use InvalidArgumentException;
use PHPUnit\Framework\Attributes\DataProvider;
use Tests\TestCase;

class HostnameTest extends TestCase
{
    #[DataProvider('validHostnames')]
    public function test_valid_hostnames_are_normalized(string $input, string $expected): void
    {
        $this->assertSame($expected, Hostname::normalize($input));
    }

    /**
     * @return array<string, array{string, string}>
     */
    public static function validHostnames(): array
    {
        return [
            'lowercase' => ['App.Example.com', 'app.example.com'],
            'trailing dot' => ['app.example.com.', 'app.example.com'],
            'subdomain' => ['a.b.example.co.uk', 'a.b.example.co.uk'],
            'hyphen' => ['my-app.example.com', 'my-app.example.com'],
        ];
    }

    #[DataProvider('invalidHostnames')]
    public function test_invalid_hostnames_are_rejected(string $hostname): void
    {
        $this->expectException(InvalidArgumentException::class);

        Hostname::normalize($hostname);
    }

    /**
     * @return array<string, array{string}>
     */
    public static function invalidHostnames(): array
    {
        return [
            'empty' => [''],
            'single label' => ['localhost'],
            'wildcard' => ['*.example.com'],
            'scheme' => ['https://example.com'],
            'port' => ['example.com:8080'],
            'path' => ['example.com/app'],
            'space' => ['exa mple.com'],
            'control' => ["exam\x01ple.com"],
            'leading hyphen label' => ['-bad.example.com'],
            'underscore' => ['bad_host.example.com'],
            'numeric tld' => ['example.123'],
        ];
    }
}
