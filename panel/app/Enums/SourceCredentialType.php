<?php

namespace App\Enums;

enum SourceCredentialType: string
{
    case HttpsToken = 'https_token';
    case SshDeployKey = 'ssh_deploy_key';
}
