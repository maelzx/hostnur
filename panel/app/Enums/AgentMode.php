<?php

namespace App\Enums;

enum AgentMode: string
{
    case Unavailable = 'unavailable';
    case Fake = 'fake';
    case Real = 'real';
}
