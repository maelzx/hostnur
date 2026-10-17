<?php

namespace App\Livewire\Panel;

use App\Models\User;
use App\Services\ServerService;
use Illuminate\Contracts\View\View;
use Illuminate\Support\Str;
use Livewire\Attributes\Layout;
use Livewire\Component;

#[Layout('layouts.panel')]
class FirewallIndex extends Component
{
    public int $port = 0;

    public string $protocol = 'tcp';

    public string $source = 'any';

    public string $action = 'allow';

    public bool $https_only = false;

    public string $ruleKey = '';

    public string $webKey = '';

    public function mount(): void
    {
        $this->ruleKey = (string) Str::uuid();
        $this->webKey = (string) Str::uuid();
    }

    public function addRule(ServerService $service): void
    {
        $task = $service->addFirewallRule($this->actor(), $this->port, $this->protocol, $this->source, $this->action, (string) Str::uuid());

        session()->flash('status', 'Firewall rule change requested.');
        $this->redirect(route('panel.tasks.show', ['task' => $task->getKey()]));
    }

    public function removeRule(ServerService $service, int $port, string $protocol, string $source, string $action): void
    {
        $task = $service->removeFirewallRule($this->actor(), $port, $protocol, $source, $action, (string) Str::uuid());

        session()->flash('status', 'Firewall rule removal requested.');
        $this->redirect(route('panel.tasks.show', ['task' => $task->getKey()]));
    }

    public function setWeb(ServerService $service): void
    {
        $task = $service->setWebHttpsOnly($this->actor(), $this->https_only, (string) Str::uuid());

        session()->flash('status', 'Web access policy requested.');
        $this->redirect(route('panel.tasks.show', ['task' => $task->getKey()]));
    }

    public function render(ServerService $service): View
    {
        return view('livewire.panel.firewall-index', [
            'snapshot' => $service->snapshot(),
            'staleAfter' => (int) config('hostnur.dashboard.stale_after_seconds', 120),
        ]);
    }

    private function actor(): User
    {
        return auth()->user();
    }
}
