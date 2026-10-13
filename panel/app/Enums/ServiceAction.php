<?php

namespace App\Enums;

enum ServiceAction: string
{
    case Start = 'start';
    case Stop = 'stop';
    case Restart = 'restart';
}
