<?php

use App\Http\Controllers\HomeController;
use App\Livewire\Auth\Login;
use App\Livewire\Auth\MfaChallenge;
use App\Livewire\Auth\MfaEnrollment;
use App\Livewire\Panel\ApplicationIndex;
use App\Livewire\Panel\ApplicationOperations;
use App\Livewire\Panel\ApplicationShow;
use App\Livewire\Panel\AuditIndex;
use App\Livewire\Panel\AuditShow;
use App\Livewire\Panel\ConfirmPassword;
use App\Livewire\Panel\Dashboard;
use App\Livewire\Panel\DeploymentIndex;
use App\Livewire\Panel\DeploymentShow;
use App\Livewire\Panel\SecuritySettings;
use App\Livewire\Panel\TaskIndex;
use App\Livewire\Panel\TaskShow;
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
    Route::get('/panel/tasks', TaskIndex::class)->name('panel.tasks.index');
    Route::get('/panel/tasks/{task}', TaskShow::class)->name('panel.tasks.show');
    Route::get('/panel/applications', ApplicationIndex::class)->name('panel.applications.index');
    Route::get('/panel/applications/{application}', ApplicationShow::class)->name('panel.applications.show');
    Route::get('/panel/audit', AuditIndex::class)->name('panel.audit.index');
    Route::get('/panel/audit/{event}', AuditShow::class)->name('panel.audit.show');
    Route::get('/panel/deployments', DeploymentIndex::class)->name('panel.deployments.index');
    Route::get('/panel/deployments/{deployment}', DeploymentShow::class)->name('panel.deployments.show');
});

Route::middleware(['auth:web', 'administrator', 'mfa', 'recent-auth'])->group(function (): void {
    Route::get('/panel/security', SecuritySettings::class)->name('panel.security');
    Route::get('/panel/applications/{application}/operations', ApplicationOperations::class)->name('panel.applications.operations');
});
