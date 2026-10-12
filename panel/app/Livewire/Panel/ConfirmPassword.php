<?php

namespace App\Livewire\Panel;

use App\Models\User;
use App\Services\RecentAuthentication;
use Illuminate\Contracts\View\View;
use Illuminate\Support\Facades\Hash;
use Livewire\Attributes\Layout;
use Livewire\Component;

#[Layout('layouts.app')]
class ConfirmPassword extends Component
{
    public string $password = '';

    public function confirm(): void
    {
        $user = $this->administrator();

        $this->validate([
            'password' => ['required', 'string'],
        ]);

        if (! Hash::check($this->password, $user->password)) {
            $this->addError('password', 'The provided password does not match our records.');

            return;
        }

        app(RecentAuthentication::class)->confirm($user);

        $this->password = '';

        $this->redirectIntended(route('panel.security'));
    }

    public function render(): View
    {
        return view('livewire.panel.confirm-password');
    }

    private function administrator(): User
    {
        $user = auth()->user();

        if (! $user instanceof User || $user->is_admin !== true) {
            abort(403);
        }

        return $user;
    }
}
