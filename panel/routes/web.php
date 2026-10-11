<?php

use App\Http\Controllers\HomeController;
use App\Livewire\Auth\Login;
use App\Livewire\Auth\MfaEnrollment;
use App\Livewire\Panel\Dashboard;
use Illuminate\Support\Facades\Route;

Route::get('/', HomeController::class)->name('home');

Route::get('/login', Login::class)
    ->middleware('guest:web')
    ->name('login');

Route::middleware(['auth:web', 'administrator'])->group(function (): void {
    Route::get('/panel', Dashboard::class)->name('panel.dashboard');
    Route::get('/panel/security/mfa', MfaEnrollment::class)->name('panel.security.mfa');
});
