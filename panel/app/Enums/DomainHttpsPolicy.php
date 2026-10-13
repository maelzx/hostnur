<?php

namespace App\Enums;

enum DomainHttpsPolicy: string
{
    case Auto = 'auto';
    case Required = 'required';
    case Disabled = 'disabled';
}
