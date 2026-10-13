<?php

namespace App\Agent\Exceptions;

use RuntimeException;

/**
 * The local agent transport failed (connect, timeout, framing or JSON error).
 */
class AgentTransportException extends RuntimeException {}
