<?php

namespace App\Livewire\Panel;

use Illuminate\Contracts\View\View;
use Livewire\Attributes\Layout;
use Livewire\Component;

#[Layout('layouts.app')]
class Dashboard extends Component
{
    public function render(): View
    {
        return view('livewire.panel.dashboard');
    }
}
