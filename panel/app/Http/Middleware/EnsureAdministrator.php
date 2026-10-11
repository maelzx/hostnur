<?php

namespace App\Http\Middleware;

use Closure;
use Illuminate\Http\Request;
use Symfony\Component\HttpFoundation\Response;

class EnsureAdministrator
{
    /**
     * Deny any authenticated account that is not an administrator.
     *
     * The administrator flag is read from the current database user on every
     * request, so removing the role takes effect immediately even for an
     * already-authenticated session.
     */
    public function handle(Request $request, Closure $next): Response
    {
        $user = $request->user();

        if ($user === null || $user->is_admin !== true) {
            abort(403);
        }

        return $next($request);
    }
}
