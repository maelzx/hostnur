<?php

namespace App\Livewire\Auth;

use App\Models\User;
use Illuminate\Contracts\View\View;
use Livewire\Attributes\Layout;
use Livewire\Component;

#[Layout('layouts.app')]
class MfaChallenge extends Component
{
    public function mount(): void
    {
        $challengedUserId = session('login.id');

        if ($challengedUserId === null || User::query()->whereKey($challengedUserId)->doesntExist()) {
            $this->redirect(route('login'));
        }
    }

    public function render(): View
    {
        return view('livewire.auth.mfa-challenge');
    }
}
