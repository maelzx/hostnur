<?php

namespace App\Livewire\Panel;

use App\Models\Application;
use App\Models\ScheduledJob;
use App\Models\User;
use App\Services\ScheduleService;
use Illuminate\Contracts\View\View;
use Illuminate\Support\Str;
use Livewire\Attributes\Layout;
use Livewire\Component;
use Livewire\WithPagination;

#[Layout('layouts.panel')]
class ScheduleIndex extends Component
{
    use WithPagination;

    public string $applicationId = '';

    public string $name = '';

    public string $expression = '0 0 * * *';

    public string $timezone = 'UTC';

    public string $command_reference = '';

    public bool $enabled = true;

    public ?string $editJobId = null;

    public string $applyKey = '';

    public function mount(): void
    {
        $this->applyKey = (string) Str::uuid();
    }

    public function edit(string $jobId): void
    {
        $job = ScheduledJob::query()->findOrFail($jobId);
        $this->editJobId = $job->getKey();
        $this->applicationId = $job->application_id;
        $this->name = $job->name;
        $this->expression = $job->expression;
        $this->timezone = $job->timezone;
        $this->command_reference = $job->command_reference;
        $this->enabled = $job->enabled;
    }

    public function save(ScheduleService $service): void
    {
        $application = Application::query()->findOrFail($this->applicationId);
        $this->authorize('update', $application);

        $job = $this->editJobId !== null ? ScheduledJob::query()->findOrFail($this->editJobId) : null;

        $service->save($this->actor(), $application, [
            'name' => $this->name,
            'expression' => $this->expression,
            'timezone' => $this->timezone,
            'command_reference' => $this->command_reference,
            'enabled' => $this->enabled,
        ], $job);

        $this->reset('name', 'command_reference', 'editJobId');
        $this->expression = '0 0 * * *';
        session()->flash('status', 'Schedule saved. No cron/systemd entry was installed.');
        $this->redirect(route('panel.schedules.index'));
    }

    public function apply(ScheduleService $service, string $jobId): void
    {
        $job = ScheduledJob::query()->findOrFail($jobId);
        $this->authorize('update', $job->application);
        $task = $service->apply($this->actor(), $job, $this->applyKey);

        session()->flash('status', 'Schedule apply requested.');
        $this->redirect(route('panel.tasks.show', ['task' => $task->getKey()]));
    }

    public function delete(ScheduleService $service, string $jobId): void
    {
        $job = ScheduledJob::query()->findOrFail($jobId);
        $this->authorize('update', $job->application);
        $service->delete($this->actor(), $job);

        session()->flash('status', 'Schedule deleted.');
        $this->redirect(route('panel.schedules.index'));
    }

    public function render(): View
    {
        return view('livewire.panel.schedule-index', [
            'applications' => Application::query()->whereNull('archived_at')->orderBy('name')->get(),
            'jobs' => ScheduledJob::query()->with('application')->latest('created_at')->paginate(20),
        ]);
    }

    private function actor(): User
    {
        return auth()->user();
    }
}
