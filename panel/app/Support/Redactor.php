<?php

namespace App\Support;

use Illuminate\Support\Str;

/**
 * Reduces the risk of persisting secrets in task events, audit metadata or task
 * error messages: structured allowlists plus pattern redaction before storage.
 * It never renders raw output as HTML.
 */
final class Redactor
{
    public const PLACEHOLDER = '[redacted]';

    /**
     * @var list<string>
     */
    private const PATTERNS = [
        '/-----BEGIN [A-Z ]+-----.*?-----END [A-Z ]+-----/s',
        '/\b(password|passwd|secret|token|api[_-]?key|authorization|recovery[_-]?code|private[_-]?key)\b\s*[:=]\s*\S+/i',
        '/\bBearer\s+[A-Za-z0-9\-._~+\/]+=*/i',
        '/\b[A-Za-z0-9+\/]{40,}={0,2}\b/',
        '/\b[0-9a-f]{32,}\b/i',
    ];

    public static function message(?string $message, int $maxLength = 500): string
    {
        if ($message === null) {
            return '';
        }

        $clean = preg_replace('/[\x00-\x08\x0B\x0C\x0E-\x1F\x7F]/u', '', $message) ?? '';

        foreach (self::PATTERNS as $pattern) {
            $clean = preg_replace($pattern, self::PLACEHOLDER, $clean) ?? $clean;
        }

        return Str::limit(trim($clean), $maxLength, '…[truncated]');
    }

    /**
     * Keep only allowlisted keys with scalar (or list of scalar) values, redacting
     * each string. Non-allowlisted keys are dropped entirely.
     *
     * @param  array<string, mixed>  $data
     * @param  list<string>  $allow
     * @return array<string, mixed>
     */
    public static function metadata(array $data, array $allow): array
    {
        $clean = [];

        foreach ($allow as $key) {
            if (! array_key_exists($key, $data)) {
                continue;
            }

            $value = $data[$key];

            if (is_string($value)) {
                $clean[$key] = self::message($value, 255);
            } elseif (is_int($value) || is_float($value) || is_bool($value) || $value === null) {
                $clean[$key] = $value;
            }
        }

        return $clean;
    }
}
