<?php

namespace App\Http\Controllers;

use Illuminate\Http\RedirectResponse;
use Illuminate\Http\Request;

class HomeController extends Controller
{
    /**
     * Send guests to the login screen and authorized administrators to the panel.
     *
     * The root route no longer renders the public development scaffold, so no
     * future authenticated panel data is reachable without a session.
     */
    public function __invoke(Request $request): RedirectResponse
    {
        return $request->user() !== null
            ? redirect()->route('panel.dashboard')
            : redirect()->route('login');
    }
}
