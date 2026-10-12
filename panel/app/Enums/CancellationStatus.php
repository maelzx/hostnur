<?php

namespace App\Enums;

enum CancellationStatus: string
{
    case Requested = 'requested';
    case Unsupported = 'unsupported';
    case AlreadyFinished = 'already_finished';
}
