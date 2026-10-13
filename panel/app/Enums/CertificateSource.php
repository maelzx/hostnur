<?php

namespace App\Enums;

enum CertificateSource: string
{
    case ManagedHttp01 = 'managed_http01';
    case UserSupplied = 'user_supplied';
}
