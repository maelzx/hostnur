<?php

namespace App\Enums;

enum BackupScope: string
{
    case Application = 'application';
    case Server = 'server';
}
