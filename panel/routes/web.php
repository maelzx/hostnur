<?php

use App\Http\Controllers\HomeController;
use App\Livewire\Auth\Login;
use App\Livewire\Auth\MfaChallenge;
use App\Livewire\Auth\MfaEnrollment;
use App\Livewire\Panel\ConfirmPassword;
use App\Livewire\Panel\Dashboard;
use App\Livewire\Panel\SecuritySettings;
use Illuminate\Support\Facades\Route;

Route::get('/', HomeController::class)->name('home');

Route::get('/login', Login::class)
    ->middleware('guest:web')
    ->name('login');

Route::get('/two-factor-challenge', MfaChallenge::class)
    ->middleware('guest:web')
    ->name('two-factor.login');

Route::middleware(['auth:web', 'administrator'])->group(function (): void {
    Route::get('/panel/security/mfa', MfaEnrollment::class)->name('panel.security.mfa');
    Route::get('/user/confirm-password', ConfirmPassword::class)->name('password.confirm');
});

Route::middleware(['auth:web', 'administrator', 'mfa'])->group(function (): void {
    Route::get('/panel', Dashboard::class)->name('panel.dashboard');
});

Route::middleware(['auth:web', 'administrator', 'mfa', 'recent-auth'])->group(function (): void {
    Route::get('/panel/security', SecuritySettings::class)->name('panel.security');
});
