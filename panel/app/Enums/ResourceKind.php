<?php

namespace App\Enums;

enum ResourceKind: string
{
    case Server = 'server';
    case Application = 'application';
    case Service = 'service';
    case Package = 'package';
    case Schedule = 'schedule';
    case Domain = 'domain';
    case Certificate = 'certificate';
    case Database = 'database';
    case Backup = 'backup';
}
