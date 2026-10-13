<?php

namespace App\Enums;

enum FakeScenario: string
{
    case Success = 'success';
    case Failure = 'failure';
    case Timeout = 'timeout';
}
