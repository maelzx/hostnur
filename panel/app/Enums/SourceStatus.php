<?php

namespace App\Enums;

enum SourceStatus: string
{
    case NotChecked = 'not_checked';
    case Checked = 'checked';
}
