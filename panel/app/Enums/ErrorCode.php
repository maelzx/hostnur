<?php

namespace App\Enums;

enum ErrorCode: string
{
    case AgentUnavailable = 'agent_unavailable';
    case UnsupportedOperation = 'unsupported_operation';
    case UnsupportedProtocol = 'unsupported_protocol';
    case InvalidParameters = 'invalid_parameters';
    case ResourceNotFound = 'resource_not_found';
    case Conflict = 'conflict';
    case Timeout = 'timeout';
    case ExecutionUnknown = 'execution_unknown';
    case PermissionDenied = 'permission_denied';
    case OperationFailed = 'operation_failed';
}
