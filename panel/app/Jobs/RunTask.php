<?php

namespace App\Jobs;

use App\Services\TaskService;
use Illuminate\Contracts\Queue\ShouldQueue;
use Illuminate\Foundation\Queue\Queueable;

/**
 * Carries only the task UUID. All durable state lives in the tasks tables, so the
 * job can be safely duplicated and a later process can inspect progress.
 */
class RunTask implements ShouldQueue
{
    use Queueable;

    public int $tries = 3;

    public int $timeout = 30;

    public function __construct(
        public readonly string $taskId,
    ) {}

    public function handle(TaskService $service): void
    {
        $service->run($this->taskId);
    }
}
