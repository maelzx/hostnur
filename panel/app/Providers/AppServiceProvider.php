<?php

namespace App\Providers;

use App\Http\Middleware\EnsureAdministrator;
use App\Http\Middleware\EnsureMfaVerified;
use Illuminate\Support\ServiceProvider;
use Livewire\Livewire;

class AppServiceProvider extends ServiceProvider
{
    /**
     * Register any application services.
     */
    public function register(): void
    {
        //
    }

    /**
     * Bootstrap any application services.
     */
    public function boot(): void
    {
        // Re-run Hostnur's server-side gates on Livewire update requests, which
        // otherwise only traverse the web middleware group. Persistence applies
        // these only to routes that already declare them.
        Livewire::addPersistentMiddleware([
            EnsureAdministrator::class,
            EnsureMfaVerified::class,
        ]);
    }
}
