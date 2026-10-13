<?php

namespace App\Enums;

enum AuditAction: string
{
    case TaskRequested = 'task.requested';
    case TaskApproved = 'task.approved';
    case TaskDenied = 'task.denied';
    case TaskStarted = 'task.started';
    case TaskSucceeded = 'task.succeeded';
    case TaskFailed = 'task.failed';
    case TaskUnknown = 'task.unknown';
    case TaskCancelled = 'task.cancelled';
    case TaskCancelRequested = 'task.cancel_requested';
    case TaskRetried = 'task.retried';
    case TaskApprovalExpired = 'task.approval_expired';

    case AuthLogin = 'auth.login';
    case AuthLogout = 'auth.logout';
    case AuthLoginFailed = 'auth.login_failed';
    case AuthLockout = 'auth.lockout';
    case AuthMfaEnrolled = 'auth.mfa_enrolled';
    case AuthMfaConfirmed = 'auth.mfa_confirmed';
    case AuthMfaDisabled = 'auth.mfa_disabled';
    case AuthMfaChallenged = 'auth.mfa_challenged';
    case AuthMfaVerified = 'auth.mfa_verified';
    case AuthRecoveryCodeUsed = 'auth.recovery_code_used';
    case AuthPasswordChanged = 'auth.password_changed';
    case AuthRecoveryCodesRegenerated = 'auth.recovery_codes_regenerated';
    case AuthAdminBootstrapped = 'auth.admin_bootstrapped';
    case AuthAdminRecovered = 'auth.admin_recovered';

    case ApplicationCreated = 'application.created';
    case ApplicationUpdated = 'application.updated';
    case ApplicationArchived = 'application.archived';
    case ApplicationSourceUpdated = 'application.source_updated';
    case ApplicationSecretSet = 'application.secret_set';
    case ApplicationSecretDeleted = 'application.secret_deleted';
    case ApplicationDomainAdded = 'application.domain_added';
    case ApplicationDomainRemoved = 'application.domain_removed';
    case SourceCredentialStored = 'application.credential_stored';
    case SourceCredentialRotated = 'application.credential_rotated';

    case ApplicationDeployRequested = 'application.deploy_requested';
    case ApplicationRestartRequested = 'application.restart_requested';
    case ApplicationRollbackRequested = 'application.rollback_requested';
}
