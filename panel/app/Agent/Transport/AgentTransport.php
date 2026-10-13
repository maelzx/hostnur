<?php

namespace App\Agent\Transport;

use App\Agent\Exceptions\AgentTransportException;

interface AgentTransport
{
    /**
     * Send a request envelope and return the decoded response object.
     *
     * Implementations must fail closed: a missing endpoint, timeout, malformed
     * frame or invalid JSON raises {@see AgentTransportException} rather than
     * returning a partial or fabricated result.
     *
     * @param  array<string, mixed>  $payload
     * @return array<string, mixed>
     */
    public function request(array $payload): array;
}
