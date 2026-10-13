<?php

namespace App\Support;

use InvalidArgumentException;

/**
 * Normalizes a bare ASCII/punycode hostname. Rejects schemes, ports, paths,
 * wildcards, control characters and malformed labels.
 */
final class Hostname
{
    public static function normalize(string $hostname): string
    {
        $hostname = trim($hostname);

        if ($hostname === '' || mb_strlen($hostname) > 253) {
            throw new InvalidArgumentException('The hostname is required and must be at most 253 characters.');
        }

        if (preg_match('/[\x00-\x1F\x7F\s]/', $hostname) === 1) {
            throw new InvalidArgumentException('The hostname must not contain spaces or control characters.');
        }

        if (str_contains($hostname, '/') || str_contains($hostname, ':') || str_contains($hostname, '*')) {
            throw new InvalidArgumentException('Enter a bare hostname without a scheme, port, path or wildcard.');
        }

        $hostname = rtrim(strtolower($hostname), '.');

        if (preg_match('/[^\x20-\x7E]/', $hostname) === 1) {
            if (! function_exists('idn_to_ascii')) {
                throw new InvalidArgumentException('Internationalized domain names are not supported on this server.');
            }

            $ascii = idn_to_ascii($hostname, IDNA_DEFAULT, INTL_IDNA_VARIANT_UTS46);

            if ($ascii === false) {
                throw new InvalidArgumentException('The hostname is not a valid internationalized domain name.');
            }

            $hostname = strtolower($ascii);
        }

        $labels = explode('.', $hostname);

        if (count($labels) < 2) {
            throw new InvalidArgumentException('The hostname must include a domain, for example app.example.com.');
        }

        foreach ($labels as $label) {
            if ($label === '' || strlen($label) > 63 || preg_match('/^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/', $label) !== 1) {
                throw new InvalidArgumentException('The hostname contains an invalid label.');
            }
        }

        if (preg_match('/^[a-z]{2,}$/', (string) end($labels)) !== 1) {
            throw new InvalidArgumentException('The hostname suffix is invalid.');
        }

        return $hostname;
    }
}
