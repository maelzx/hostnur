<?php

namespace App\Support;

use InvalidArgumentException;

/**
 * Normalizes and validates planned Git repository URLs (HTTPS or verified-key
 * SSH). It never fetches anything and rejects embedded credentials, local/ext
 * schemes, control characters, query/fragment tricks and option-like paths.
 */
final class GitRepositoryUrl
{
    public static function normalize(string $url): string
    {
        $url = trim($url);

        if ($url === '' || mb_strlen($url) > 2048) {
            throw new InvalidArgumentException('The repository URL is required and must be at most 2048 characters.');
        }

        if (preg_match('/[\x00-\x1F\x7F\s]/', $url) === 1) {
            throw new InvalidArgumentException('The repository URL must not contain spaces or control characters.');
        }

        if (preg_match('#^(file|ext|data|php|phar|gopher):#i', $url) === 1
            || str_starts_with($url, '/')
            || str_starts_with($url, './')
            || str_starts_with($url, '../')
            || str_starts_with($url, '~')) {
            throw new InvalidArgumentException('Local or unsupported repository locations are not allowed.');
        }

        // scp-like SSH form: git@host:owner/repo.git
        if (preg_match('#^git@([a-z0-9](?:[a-z0-9-]*[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]*[a-z0-9])?)+):([A-Za-z0-9._/-]+)$#', $url, $matches) === 1) {
            return 'git@'.strtolower($matches[1]).':'.$matches[2];
        }

        $parts = parse_url($url);

        if ($parts === false || ! isset($parts['scheme'], $parts['host'], $parts['path'])) {
            throw new InvalidArgumentException('The repository URL could not be parsed.');
        }

        $scheme = strtolower($parts['scheme']);

        if (! in_array($scheme, ['https', 'ssh'], true)) {
            throw new InvalidArgumentException('Only https and ssh repository URLs are allowed.');
        }

        if (isset($parts['query']) || isset($parts['fragment'])) {
            throw new InvalidArgumentException('Repository URLs must not contain a query string or fragment.');
        }

        if (isset($parts['port'])) {
            throw new InvalidArgumentException('Repository URLs must not specify a custom port.');
        }

        if (isset($parts['pass'])) {
            throw new InvalidArgumentException('Repository URLs must not embed credentials.');
        }

        $user = $parts['user'] ?? null;

        if ($scheme === 'https' && $user !== null) {
            throw new InvalidArgumentException('HTTPS repository URLs must not embed credentials.');
        }

        if ($scheme === 'ssh' && $user !== null && $user !== 'git') {
            throw new InvalidArgumentException('SSH repository URLs must use the git user.');
        }

        $host = strtolower($parts['host']);

        if (preg_match('/^[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?$/', $host) !== 1) {
            throw new InvalidArgumentException('The repository host is invalid.');
        }

        $path = ltrim($parts['path'], '/');

        if ($path === '' || str_starts_with($path, '-') || str_contains($path, '..')) {
            throw new InvalidArgumentException('The repository path is invalid.');
        }

        $prefix = $scheme === 'ssh' ? 'ssh://git@' : 'https://';

        return $prefix.$host.'/'.$path;
    }
}
