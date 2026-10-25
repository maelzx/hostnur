<?php

namespace App\Console\Commands;

use App\Actions\Applications\CreateApplication;
use App\Enums\TaskStatus;
use App\Models\Application;
use App\Models\Deployment;
use App\Models\Task;
use App\Models\User;
use App\Services\CertificateService;
use App\Services\DatabaseService;
use App\Services\DeploymentService;
use App\Services\TaskService;
use Illuminate\Console\Attributes\Description;
use Illuminate\Console\Attributes\Signature;
use Illuminate\Console\Command;
use Illuminate\Support\Facades\Http;
use Illuminate\Support\Sleep;
use Illuminate\Support\Str;
use Illuminate\Validation\ValidationException;
use RuntimeException;
use Throwable;

/**
 * Drive the panel's own create → provision → deploy → certificate →
 * redeploy/rollback path against the configured agent, as an administrator,
 * without a browser. It calls the same action and services as the UI (only the
 * login/MFA layer is skipped, which shell access on the host already
 * supersedes), runs each task through the task pipeline and waits for its
 * outcome, then checks the application over HTTP. Meant for disposable
 * validation hosts: it creates a real application and changes the server.
 */
#[Signature('hostnur:smoke
    {--repo= : Public HTTPS Git repository of a Laravel application}
    {--ref=main : Branch, tag or commit to deploy}
    {--hostname= : Domain to serve the application on (needed for the HTTP check and --certificate)}
    {--php= : PHP runtime version, e.g. 8.4 (defaults to the panel default)}
    {--health-path=up : Health check path, relative to the application (e.g. up)}
    {--database : Provision a MariaDB database before deploying}
    {--migrate : Run migrations during deploy (acknowledges that a recovery point exists)}
    {--certificate : Request a Let\'s Encrypt certificate after the first deploy}
    {--rollback : Deploy a second time, then roll back to the first release}
    {--name= : Application name (default smoke-<timestamp>)}
    {--actor= : Administrator email to act as (default: the only administrator)}
    {--wait=900 : Seconds to wait for each task}
    {--force : Do not ask for confirmation}')]
#[Description('Exercise the panel → task → agent path end to end on a validation host')]
class SmokeTest extends Command
{
    /** @var list<array{0: string, 1: string, 2: string}> */
    private array $steps = [];

    public function handle(
        CreateApplication $create,
        DeploymentService $deployments,
        DatabaseService $databases,
        CertificateService $certificates,
        TaskService $tasks,
    ): int {
        $mode = (string) config('hostnur.agent_mode');

        if ($mode === 'unavailable') {
            $this->error('The agent mode is "unavailable"; set HOSTNUR_AGENT_MODE to real (or fake) first.');

            return self::FAILURE;
        }

        $repository = trim((string) $this->option('repo'));

        if ($repository === '') {
            $this->error('Pass --repo with the repository to deploy.');

            return self::FAILURE;
        }

        $hostname = trim((string) $this->option('hostname'));

        if ($this->option('certificate') && $hostname === '') {
            $this->error('--certificate needs --hostname.');

            return self::FAILURE;
        }

        try {
            $actor = $this->actor();
        } catch (RuntimeException $exception) {
            $this->error($exception->getMessage());

            return self::FAILURE;
        }

        $name = trim((string) $this->option('name')) ?: 'smoke-'.now()->format('YmdHis');
        $slug = Str::slug($name);

        if (! $this->option('force') && ! $this->confirm(
            "Create application \"{$name}\" and run real operations in {$mode} mode as {$actor->email}?"
        )) {
            $this->line('Cancelled.');

            return self::FAILURE;
        }

        $passed = $this->runSteps($create, $deployments, $databases, $certificates, $tasks, $actor, $name, $slug, $repository, $hostname);

        $this->newLine();
        $this->table(['Step', 'Result', 'Detail'], $this->steps);

        return $passed ? self::SUCCESS : self::FAILURE;
    }

    private function runSteps(
        CreateApplication $create,
        DeploymentService $deployments,
        DatabaseService $databases,
        CertificateService $certificates,
        TaskService $tasks,
        User $actor,
        string $name,
        string $slug,
        string $repository,
        string $hostname,
    ): bool {
        try {
            $application = $create->handle($actor, [
                'name' => $name,
                'slug' => $slug,
                'provider' => 'laravel',
                'runtime' => 'php',
                'runtime_version' => $this->option('php') ?: null,
                'repository_url' => $repository,
                'git_ref' => (string) $this->option('ref'),
                'configuration' => [
                    'runtime' => 'php',
                    'runtime_version' => $this->option('php') ?: null,
                    'build_command' => null,
                    'start_command' => null,
                    'workers' => 0,
                    'scheduler' => false,
                    'health_path' => ltrim((string) $this->option('health-path'), '/'),
                    'persistent_paths' => [],
                ],
                'hostname' => $hostname,
                'database' => $this->option('database') ? [
                    'engine' => 'mariadb',
                    'name' => Str::limit(str_replace('-', '_', $slug), 60, ''),
                    'username' => Str::limit(str_replace('-', '_', $slug), 60, ''),
                    'key' => (string) Str::uuid(),
                ] : null,
            ]);
        } catch (ValidationException $exception) {
            return $this->record('Create application', false, $this->validationMessage($exception));
        }
        $this->record('Create application', true, "{$application->slug} ({$application->getKey()})");

        if ($this->option('database')) {
            $request = $application->databaseRequests()->latest()->first();
            $task = $this->await($tasks, $request->task, 'Provision database');
            $databases->syncState($request->refresh());

            if (! $this->record('Provision database', $task->status === TaskStatus::Succeeded, $this->outcome($task))) {
                return false;
            }
        }

        $first = $this->deploy($deployments, $tasks, $actor, $application, 'Deploy');

        if ($first === null) {
            return false;
        }

        if ($this->option('certificate')) {
            try {
                $certificate = $certificates->requestManaged($actor, $application->domains()->firstOrFail(), (string) Str::uuid());
            } catch (ValidationException $exception) {
                return $this->record('Request certificate', false, $this->validationMessage($exception));
            }
            $task = $this->await($tasks, $certificate->task, 'Request certificate');
            $certificates->syncState($certificate->refresh());

            if (! $this->record('Request certificate', $task->status === TaskStatus::Succeeded, $this->outcome($task))) {
                return false;
            }
        }

        if ($hostname !== '' && ! $this->checkHttp($hostname, 'HTTP check')) {
            return false;
        }

        if ($this->option('rollback')) {
            if ($this->deploy($deployments, $tasks, $actor, $application, 'Redeploy') === null) {
                return false;
            }

            try {
                $rollback = $deployments->rollback($actor, $application->refresh(), $first->getKey(), (string) Str::uuid());
            } catch (Throwable $exception) {
                return $this->record('Roll back to first release', false, $exception->getMessage());
            }
            $task = $this->await($tasks, $rollback->task, 'Roll back to first release');
            $rollback = $deployments->refresh($rollback->refresh());

            if (! $this->record('Roll back to first release', $task->status === TaskStatus::Succeeded, $this->outcome($task, $rollback))) {
                return false;
            }

            if ($hostname !== '' && ! $this->checkHttp($hostname, 'HTTP check after rollback')) {
                return false;
            }
        }

        return true;
    }

    private function deploy(DeploymentService $deployments, TaskService $tasks, User $actor, Application $application, string $label): ?Deployment
    {
        $migrate = (bool) $this->option('migrate');

        try {
            $deployment = $deployments->deploy($actor, $application->refresh(), (string) Str::uuid(), $migrate, $migrate);
        } catch (Throwable $exception) {
            $this->record($label, false, $exception->getMessage());

            return null;
        }

        $task = $this->await($tasks, $deployment->task, $label);
        $deployment = $deployments->refresh($deployment->refresh());
        $passed = $task->status === TaskStatus::Succeeded;
        $this->record($label, $passed, $this->outcome($task, $deployment));

        return $passed ? $deployment : null;
    }

    /**
     * Run the task through the pipeline until it settles, echoing its events.
     * `TaskService::run` claims a lease, so a queue worker on the same host
     * running the task concurrently is harmless.
     */
    private function await(TaskService $tasks, Task $task, string $label): Task
    {
        $this->line("<info>→</info> {$label}");
        $deadline = now()->addSeconds(max(1, (int) $this->option('wait')));
        $seen = 0;

        while (true) {
            $tasks->run($task->getKey());
            $task->refresh();

            foreach ($task->events()->where('sequence', '>', $seen)->get() as $event) {
                $this->line("    {$event->message}");
                $seen = $event->sequence;
            }

            if ($task->status->isTerminal() || $task->status === TaskStatus::Unknown || now()->greaterThanOrEqualTo($deadline)) {
                return $task;
            }

            Sleep::for(2)->seconds();
        }
    }

    private function checkHttp(string $hostname, string $label): bool
    {
        $scheme = $this->option('certificate') ? 'https' : 'http';
        $url = $scheme.'://'.$hostname.'/'.ltrim((string) $this->option('health-path'), '/');

        try {
            $status = Http::timeout(15)->withoutRedirecting()->get($url)->status();
        } catch (Throwable $exception) {
            return $this->record($label, false, "{$url}: {$exception->getMessage()}");
        }

        return $this->record($label, $status >= 200 && $status < 400, "{$url} → {$status}");
    }

    private function outcome(Task $task, ?Deployment $deployment = null): string
    {
        $detail = $task->status->value;

        if ($task->status !== TaskStatus::Succeeded) {
            $detail .= ' — '.($task->error_code ?? 'no error code').': '.($task->error_message ?? 'no message');
            if ($task->status->isActive()) {
                $detail .= ' (still running when --wait elapsed)';
            }
        } elseif ($deployment?->release_reference !== null) {
            $detail .= " — release {$deployment->release_reference}";
        }

        return $detail;
    }

    private function record(string $step, bool $passed, string $detail): bool
    {
        $this->steps[] = [$step, $passed ? 'pass' : 'FAIL', $detail];
        $this->line(($passed ? '  <info>pass</info> ' : '  <error>FAIL</error> ').$detail);

        return $passed;
    }

    private function validationMessage(ValidationException $exception): string
    {
        return collect($exception->errors())->flatten()->implode(' ');
    }

    private function actor(): User
    {
        $email = $this->option('actor');
        $admins = User::query()->where('is_admin', true);

        if (is_string($email) && $email !== '') {
            return $admins->where('email', $email)->first()
                ?? throw new RuntimeException("No administrator with email {$email}.");
        }

        $found = $admins->limit(2)->get();

        if ($found->count() !== 1) {
            throw new RuntimeException('Pass --actor with the administrator email to act as.');
        }

        return $found->first();
    }
}
