<?php

namespace App\Enums;

enum TaskEventLevel: string
{
    case Info = 'info';
    case Warning = 'warning';
    case Error = 'error';
}
