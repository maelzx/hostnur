<?php

namespace Tests\Feature;

use App\Enums\TaskEventLevel;
use App\Enums\TaskStatus;
use App\Models\AuditEvent;
use App\Models\Task;
use App\Models\User;
use Illuminate\Auth\Events\Failed;
use Illuminate\Auth\Events\Login;
use Illuminate\Auth\Events\Logout;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Support\Facades\DB;
use Laravel\Fortify\Events\RecoveryCodeReplaced;
use Laravel\Fortify\Events\TwoFactorAuthenticationConfirmed;
use Tests\TestCase;

class TaskAuditUiTest extends TestCase
{
    use RefreshDatabase;

    private function admin(): User
    {
        return User::factory()->create(['email' => 'admin@example.com', 'is_admin' => true]);
    }

    private function confirmedAdmin(): User
    {
        return User::factory()->create([
            'email' => 'admin@example.com',
            'is_admin' => true,
            'two_factor_secret' => 'encrypted',
            'two_factor_confirmed_at' => now(),
            'mfa_version' => 0,
        ]);
    }

    private function mfaProof(User $admin): array
    {
        return ['mfa_verified' => ['id' => $admin->getKey(), 'version' => $admin->mfa_version]];
    }

    private function task(User $admin): Task
    {
        return Task::create([
            'actor_id' => $admin->getKey(),
            'actor_label' => $admin->email,
            'operation' => 'application.restart',
            'resource_kind' => 'application',
            'resource_id' => '224065ac-4bc7-4ba4-a06d-8c8bca950c32',
            'mode' => 'fake',
            'status' => TaskStatus::Succeeded,
            'idempotency_key' => 'cf36a770-2abf-42e6-83cf-a0f042bcfc3c',
            'parameters' => [],
            'payload_fingerprint' => str_repeat('c', 64),
            'requested_at' => now(),
        ]);
    }

    public function test_login_and_logout_events_are_recorded(): void
    {
        $admin = $this->admin();

        event(new Login('web', $admin, false));
        event(new Logout('web', $admin));

        $this->assertDatabaseHas('audit_events', ['action' => 'auth.login', 'actor_label' => 'admin@example.com']);
        $this->assertDatabaseHas('audit_events', ['action' => 'auth.logout']);
    }

    public function test_a_failed_login_records_the_email_but_never_the_password(): void
    {
        event(new Failed('web', null, ['email' => 'someone@example.com', 'password' => 'super-secret-value']));

        $this->assertDatabaseHas('audit_events', ['action' => 'auth.login_failed', 'outcome' => 'failure']);

        $metadata = (string) DB::table('audit_events')->where('action', 'auth.login_failed')->value('metadata');
        $this->assertStringContainsString('someone@example.com', $metadata);
        $this->assertStringNotContainsString('super-secret-value', $metadata);
    }

    public function test_mfa_confirmation_is_recorded(): void
    {
        $admin = $this->admin();

        event(new TwoFactorAuthenticationConfirmed($admin));

        $this->assertDatabaseHas('audit_events', ['action' => 'auth.mfa_confirmed']);
    }

    public function test_a_recovery_code_event_never_stores_the_code(): void
    {
        $admin = $this->admin();

        event(new RecoveryCodeReplaced($admin, 'recovery-code-abcdef123456'));

        $this->assertDatabaseHas('audit_events', ['action' => 'auth.recovery_code_used']);

        foreach (DB::table('audit_events')->get() as $row) {
            $this->assertStringNotContainsString('recovery-code-abcdef123456', json_encode($row));
        }
    }

    public function test_task_event_messages_are_redacted_and_truncated(): void
    {
        $task = $this->task($this->admin());

        $event = $task->recordEvent('output', TaskEventLevel::Info, 'password=hunter2 token=abcdefghijklmnopqrstuvwxyz0123456789');
        $this->assertStringNotContainsString('hunter2', $event->message);
        $this->assertStringContainsString('[redacted]', $event->message);

        $long = $task->recordEvent('output', TaskEventLevel::Info, str_repeat('x', 2000));
        $this->assertLessThanOrEqual(520, strlen($long->message));
    }

    public function test_guests_are_redirected_from_history_pages(): void
    {
        $this->get('/panel/tasks')->assertRedirect(route('login'));
        $this->get('/panel/audit')->assertRedirect(route('login'));
    }

    public function test_non_administrators_are_forbidden_from_history_pages(): void
    {
        $user = User::factory()->create(['is_admin' => false]);

        $this->actingAs($user)->get('/panel/tasks')->assertForbidden();
        $this->actingAs($user)->get('/panel/audit')->assertForbidden();
    }

    public function test_an_administrator_can_view_task_and_audit_history(): void
    {
        $admin = $this->confirmedAdmin();
        $task = $this->task($admin);
        AuditEvent::create([
            'actor_id' => $admin->getKey(),
            'actor_label' => $admin->email,
            'action' => 'auth.login',
            'outcome' => 'success',
            'recorded_at' => now(),
        ]);

        $this->actingAs($admin)
            ->withSession($this->mfaProof($admin))
            ->get('/panel/tasks')
            ->assertOk()
            ->assertSee('application.restart');

        $this->actingAs($admin)
            ->withSession($this->mfaProof($admin))
            ->get('/panel/tasks/'.$task->getKey())
            ->assertOk()
            ->assertSee('Requested');

        $this->actingAs($admin)
            ->withSession($this->mfaProof($admin))
            ->get('/panel/audit')
            ->assertOk()
            ->assertSee('auth.login');
    }
}
