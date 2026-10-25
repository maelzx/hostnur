<?php

namespace App\Agent\Transport;

use App\Agent\Exceptions\AgentTransportException;

/**
 * Length-prefixed JSON over a local Unix stream socket.
 *
 * Each message is a four-byte unsigned big-endian length followed by UTF-8
 * JSON. One request/response per connection. The length is checked before the
 * body is read, mirroring the Rust agent's framing.
 */
final class UnixSocketAgentTransport implements AgentTransport
{
    private const MAX_MESSAGE_BYTES = 1_048_576;

    /**
     * @param  resource|null  $stream  Pre-connected stream (test seam); when
     *                                 null a fresh connection is opened per call.
     */
    public function __construct(
        private readonly string $socketPath,
        private readonly float $timeoutSeconds = 5.0,
        private $stream = null,
    ) {}

    public function request(array $payload): array
    {
        $stream = $this->stream;
        $ownsStream = false;

        if ($stream === null) {
            $stream = $this->connect();
            $ownsStream = true;
        }

        try {
            // The agent answers every request promptly (mutations are accepted
            // and run in the background), so the connection timeout always
            // applies; a request's `timeout_seconds` bounds the agent's execution.
            $this->writeFrame($stream, $payload);

            return $this->readFrame($stream);
        } finally {
            if ($ownsStream && is_resource($stream)) {
                fclose($stream);
            }
        }
    }

    /**
     * @return resource
     */
    private function connect()
    {
        $errno = 0;
        $error = '';

        $stream = @stream_socket_client(
            'unix://'.$this->socketPath,
            $errno,
            $error,
            $this->timeoutSeconds,
        );

        if ($stream === false) {
            throw new AgentTransportException(
                "Could not connect to the agent socket ({$errno})."
            );
        }

        $seconds = (int) $this->timeoutSeconds;
        $microseconds = (int) (($this->timeoutSeconds - $seconds) * 1_000_000);
        stream_set_timeout($stream, $seconds, $microseconds);
        stream_set_blocking($stream, true);

        return $stream;
    }

    /**
     * @param  resource  $stream
     * @param  array<string, mixed>  $payload
     */
    private function writeFrame($stream, array $payload): void
    {
        $body = json_encode($payload, JSON_THROW_ON_ERROR | JSON_UNESCAPED_SLASHES);
        $length = strlen($body);

        if ($length === 0 || $length > self::MAX_MESSAGE_BYTES) {
            throw new AgentTransportException('The request exceeds the maximum message size.');
        }

        $written = 0;
        $frame = pack('N', $length).$body;

        while ($written < strlen($frame)) {
            $chunk = @fwrite($stream, substr($frame, $written));

            if ($chunk === false || $chunk === 0) {
                throw new AgentTransportException('Failed to write the agent request.');
            }

            $written += $chunk;
        }

        fflush($stream);
    }

    /**
     * @param  resource  $stream
     * @return array<string, mixed>
     */
    private function readFrame($stream): array
    {
        $prefix = $this->readExact($stream, 4);
        $length = unpack('Nlength', $prefix)['length'];

        if ($length === 0 || $length > self::MAX_MESSAGE_BYTES) {
            throw new AgentTransportException('The agent response frame has an invalid length.');
        }

        $body = $this->readExact($stream, $length);
        $decoded = json_decode($body, true);

        if (! is_array($decoded)) {
            throw new AgentTransportException('The agent response was not a JSON object.');
        }

        return $decoded;
    }

    /**
     * @param  resource  $stream
     */
    private function readExact($stream, int $length): string
    {
        $buffer = '';

        while (strlen($buffer) < $length) {
            $chunk = @fread($stream, $length - strlen($buffer));

            if ($chunk === false || $chunk === '') {
                $meta = stream_get_meta_data($stream);

                if (($meta['timed_out'] ?? false) === true) {
                    throw new AgentTransportException('Timed out waiting for the agent response.');
                }

                throw new AgentTransportException('The agent connection closed unexpectedly.');
            }

            $buffer .= $chunk;
        }

        return $buffer;
    }
}
