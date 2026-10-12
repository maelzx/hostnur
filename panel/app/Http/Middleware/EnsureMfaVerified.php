<?php

namespace App\Http\Middleware;

use Closure;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Auth;
use Symfony\Component\HttpFoundation\Response;

class EnsureMfaVerified
{
    /**
     * Require a confirmed, currently valid MFA session proof for app screens.
     *
     * Administrators without confirmed MFA are sent to enrollment; a confirmed
     * administrator whose generation-bound proof is missing or stale is logged
     * out so the next password login starts a fresh two-factor challenge.
     */
    public function handle(Request $request, Closure $next): Response
    {
        $user = $request->user();

        if ($user === null || $user->is_admin !== true) {
            abort(403);
        }

        if (! $user->hasEnabledTwoFactorAuthentication()) {
            return redirect()->route('panel.security.mfa');
        }

        if (! $this->hasValidProof($request, $user)) {
            Auth::guard('web')->logout();
            $request->session()->invalidate();
            $request->session()->regenerateToken();

            return redirect()->route('login');
        }

        return $next($request);
    }

    private function hasValidProof(Request $request, mixed $user): bool
    {
        $proof = $request->session()->get('mfa_verified');

        return is_array($proof)
            && ($proof['id'] ?? null) === $user->getKey()
            && (string) ($proof['version'] ?? null) === (string) $user->mfa_version;
    }
}
