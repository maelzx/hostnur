<?php

namespace App\Http\Middleware;

use App\Models\User;
use App\Services\RecentAuthentication;
use Closure;
use Illuminate\Http\Request;
use Symfony\Component\HttpFoundation\Response;

class EnsureRecentAuthentication
{
    public function __construct(
        private readonly RecentAuthentication $recentAuthentication,
    ) {}

    /**
     * Require a recent, generation-bound password confirmation. Authenticated
     * administrators without one are sent to the password confirmation screen
     * with the intended URL preserved.
     */
    public function handle(Request $request, Closure $next): Response
    {
        $user = $request->user();

        if ($user instanceof User && $this->recentAuthentication->isConfirmed($user)) {
            return $next($request);
        }

        return redirect()->guest(route('password.confirm'));
    }
}
