<?php

namespace App\Enums;

enum AuditOutcome: string
{
    case Requested = 'requested';
    case Success = 'success';
    case Failure = 'failure';
    case Denied = 'denied';
}
