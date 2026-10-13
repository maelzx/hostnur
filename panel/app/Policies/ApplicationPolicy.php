<?php

namespace App\Policies;

use App\Models\Application;
use App\Models\User;

class ApplicationPolicy
{
    public function viewAny(User $user): bool
    {
        return $user->is_admin === true;
    }

    public function view(User $user, Application $application): bool
    {
        return $user->is_admin === true;
    }

    public function create(User $user): bool
    {
        return $user->is_admin === true;
    }

    public function update(User $user, Application $application): bool
    {
        return $user->is_admin === true;
    }

    public function archive(User $user, Application $application): bool
    {
        return $user->is_admin === true;
    }

    public function manageSource(User $user, Application $application): bool
    {
        return $user->is_admin === true;
    }

    public function manageSecrets(User $user, Application $application): bool
    {
        return $user->is_admin === true;
    }

    public function manageDomains(User $user, Application $application): bool
    {
        return $user->is_admin === true;
    }
}
