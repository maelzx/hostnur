<?php

namespace Tests\Feature;

use App\Models\Application;
use App\Models\Deployment;
use App\Models\User;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Support\Carbon;
use Illuminate\Support\Facades\Http;
use Illuminate\Support\Facades\Queue;
use Illuminate\Support\Sleep;
use Tests\TestCase;

class SmokeTestCommandTest extends TestCase
{
    use RefreshDatabase;

    private int $healthStatus = 200;

    protected function setUp(): void
    {
        parent::setUp();

        config(['hostnur.agent_mode' => 'fake']);
        Queue::fake();
        $this->travelTo(Carbon::parse('2026-10-25T02:00:00Z'));
        // Each wait advances the controlled clock, so fake executions finish.
        Sleep::fake(syncWithCarbon: true);
        Http::fake(fn () => Http::response('', $this->healthStatus));
        User::factory()->create(['email' => 'admin@example.com', 'is_admin' => true]);
    }

    /**
     * @return array<string, mixed>
     */
    private function smokeOptions(array $extra = []): array
    {
        return [
            '--repo' => 'https://github.com/example/app.git',
            '--hostname' => 'app.example.com',
            '--name' => 'smoke-app',
            '--force' => true,
            ...$extra,
        ];
    }

    public function test_it_creates_deploys_redeploys_and_rolls_back_through_the_panel(): void
    {
        $this->artisan('hostnur:smoke', $this->smokeOptions(['--rollback' => true]))
            ->expectsOutputToContain('Deploy')
            ->assertSuccessful();

        $application = Application::query()->where('slug', 'smoke-app')->sole();
        $this->assertSame('https://github.com/example/app.git', $application->repository_url);
        $this->assertTrue($application->domains()->where('hostname', 'app.example.com')->exists());

        $operations = Deployment::query()->where('application_id', $application->getKey())->orderBy('requested_at')->pluck('operation')->all();
        $this->assertSame(['application.deploy', 'application.deploy', 'application.rollback'], $operations);

        $first = Deployment::query()->where('application_id', $application->getKey())->where('operation', 'application.deploy')->orderBy('requested_at')->first();
        $rollback = Deployment::query()->where('operation', 'application.rollback')->sole();
        $this->assertSame(['release' => $first->release_reference], $rollback->task->parameters);

        Http::assertSentCount(2);
        Http::assertSent(fn ($request): bool => $request->url() === 'http://app.example.com/up');
    }

    public function test_it_provisions_a_database_before_deploying(): void
    {
        $this->artisan('hostnur:smoke', $this->smokeOptions(['--database' => true]))->assertSuccessful();

        $application = Application::query()->where('slug', 'smoke-app')->sole();
        $request = $application->databaseRequests()->sole();
        $this->assertSame('smoke_app', $request->database_name);
        $this->assertSame('succeeded', $request->task->status->value);
    }

    public function test_a_failed_task_stops_the_run_and_fails_the_command(): void
    {
        config(['hostnur.fake.scenario' => 'failure']);

        $this->artisan('hostnur:smoke', $this->smokeOptions(['--rollback' => true]))
            ->expectsOutputToContain('FAIL')
            ->assertFailed();

        $this->assertSame(1, Deployment::query()->count());
        Http::assertNothingSent();
    }

    public function test_a_failed_health_check_fails_the_command(): void
    {
        $this->healthStatus = 503;

        $this->artisan('hostnur:smoke', $this->smokeOptions())->assertFailed();
    }

    public function test_it_asks_for_confirmation_without_force(): void
    {
        $options = $this->smokeOptions();
        unset($options['--force']);

        $this->artisan('hostnur:smoke', $options)
            ->expectsConfirmation('Create application "smoke-app" and run real operations in fake mode as admin@example.com?', 'no')
            ->assertFailed();

        $this->assertSame(0, Application::query()->count());
    }

    public function test_it_refuses_without_an_agent_or_a_repository(): void
    {
        config(['hostnur.agent_mode' => 'unavailable']);
        $this->artisan('hostnur:smoke', $this->smokeOptions())->assertFailed();

        config(['hostnur.agent_mode' => 'fake']);
        $this->artisan('hostnur:smoke', ['--force' => true])->assertFailed();

        $this->artisan('hostnur:smoke', $this->smokeOptions(['--hostname' => null, '--certificate' => true]))->assertFailed();

        $this->assertSame(0, Application::query()->count());
    }

    public function test_it_needs_an_explicit_actor_when_several_administrators_exist(): void
    {
        User::factory()->create(['email' => 'second@example.com', 'is_admin' => true]);

        $this->artisan('hostnur:smoke', $this->smokeOptions())->assertFailed();
        $this->artisan('hostnur:smoke', $this->smokeOptions(['--actor' => 'second@example.com']))->assertSuccessful();
    }
}
