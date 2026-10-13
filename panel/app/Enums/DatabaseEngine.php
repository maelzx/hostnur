<?php

namespace App\Enums;

enum DatabaseEngine: string
{
    case MariaDb = 'mariadb';
    case PostgreSql = 'postgresql';
}
