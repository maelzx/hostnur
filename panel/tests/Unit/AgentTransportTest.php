<?php

namespace Tests\Unit;

use App\Agent\Exceptions\AgentTransportException;
use App\Agent\Transport\UnixSocketAgentTransport;
use PHPUnit\Framework\TestCase;

class AgentTransportTest extends TestCase
{
    /**
     * @return array{0: resource, 1: resource}
     */
    private function pair(): array
    {
        $pair = stream_socket_pair(STREAM_PF_UNIX, STREAM_SOCK_STREAM, 0);
        $this->assertIsArray($pair);

        return $pair;
    }

    /**
     * @param  resource  $stream
     * @param  array<string, mixed>  $payload
     */
    private function writeFrame($stream, array $payload): void
    {
        $body = json_encode($payload, JSON_THROW_ON_ERROR);
        fwrite($stream, pack('N', strlen($body)).$body);
    }

    public function test_frames_a_request_and_decodes_the_response(): void
    {
        [$client, $server] = $this->pair();

        $response = [
            'protocol_version' => 1,
            'mode' => 'real',
            'available' => true,
            'operations' => [],
            'sampled_at' => '2026-10-12T12:00:00Z',
            'reason' => null,
        ];
        $this->writeFrame($server, $response);

        $transport = new UnixSocketAgentTransport('/unused', 2.0, $client);
        $decoded = $transport->request([
            'protocol_version' => 1,
            'request_id' => '6c9dc595-fc9c-4c13-8c72-ed9a489ab9e7',
            'idempotency_key' => 'cf36a770-2abf-42e6-83cf-a0f042bcfc3c',
            'operation' => 'agent.capabilities',
            'parameters' => (object) [],
        ]);

        $this->assertSame('real', $decoded['mode']);

        // The request must be a valid length-prefixed frame on the peer end.
        $prefix = fread($server, 4);
        $this->assertSame(4, strlen($prefix));
        $length = unpack('Nlength', $prefix)['length'];
        $sent = json_decode(fread($server, $length), true, flags: JSON_THROW_ON_ERROR);
        $this->assertSame('agent.capabilities', $sent['operation']);
    }

    public function test_rejects_zero_length_frame(): void
    {
        [$client, $server] = $this->pair();
        fwrite($server, pack('N', 0));

        $transport = new UnixSocketAgentTransport('/unused', 2.0, $client);

        $this->expectException(AgentTransportException::class);
        $transport->request(['protocol_version' => 1]);
    }

    public function test_rejects_oversized_frame_before_reading_body(): void
    {
        [$client, $server] = $this->pair();
        fwrite($server, pack('N', 5_000_000));

        $transport = new UnixSocketAgentTransport('/unused', 2.0, $client);

        $this->expectException(AgentTransportException::class);
        $transport->request(['protocol_version' => 1]);
    }

    public function test_rejects_truncated_frame(): void
    {
        [$client, $server] = $this->pair();
        fwrite($server, pack('N', 10).'ab');
        fclose($server);

        $transport = new UnixSocketAgentTransport('/unused', 2.0, $client);

        $this->expectException(AgentTransportException::class);
        $transport->request(['protocol_version' => 1]);
    }

    public function test_missing_socket_fails_closed(): void
    {
        $transport = new UnixSocketAgentTransport('/nonexistent/hostnur-agent.sock', 0.5);

        $this->expectException(AgentTransportException::class);
        $transport->request(['protocol_version' => 1]);
    }
}
