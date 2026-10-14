<?php

namespace App\Enums;

enum ApplicationStatus: string
{
    case NotDeployed = 'not_deployed';
    case Deploying = 'deploying';
    case Deployed = 'deployed';
    case Failed = 'failed';
    case Cancelled = 'cancelled';
    case Archived = 'archived';

    public static function fromDeploymentState(DeploymentState $state): self
    {
        return match ($state) {
            DeploymentState::Requested, DeploymentState::Running => self::Deploying,
            DeploymentState::Succeeded => self::Deployed,
            DeploymentState::Failed => self::Failed,
            DeploymentState::Cancelled => self::Cancelled,
            DeploymentState::Unknown => self::NotDeployed,
        };
    }

    public function label(): string
    {
        return match ($this) {
            self::NotDeployed => 'Not deployed',
            self::Deploying => 'Deploying',
            self::Deployed => 'Deployed',
            self::Failed => 'Failed',
            self::Cancelled => 'Cancelled',
            self::Archived => 'Archived',
        };
    }

    public function tone(): string
    {
        return match ($this) {
            self::Deployed => 'success',
            self::Failed => 'danger',
            self::Deploying => 'info',
            self::NotDeployed => 'neutral',
            self::Cancelled, self::Archived => 'warning',
        };
    }
}
