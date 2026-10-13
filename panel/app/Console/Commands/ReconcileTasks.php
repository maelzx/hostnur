<?php

namespace App\Console\Commands;

use App\Services\TaskService;
use Illuminate\Console\Attributes\Description;
use Illuminate\Console\Attributes\Signature;
use Illuminate\Console\Command;

#[Signature('hostnur:tasks:reconcile')]
#[Description('Redispatch queued tasks, poll running tasks and recover expired leases')]
class ReconcileTasks extends Command
{
    public function handle(TaskService $service): int
    {
        $result = $service->reconcile();

        $this->info(sprintf(
            'Tasks reconciled: %d dispatched, %d expired leases recovered.',
            $result['dispatched'],
            $result['leases_expired'],
        ));

        return self::SUCCESS;
    }
}
