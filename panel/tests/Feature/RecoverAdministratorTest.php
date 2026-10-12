<?php

namespace Tests\Feature;

use App\Models\User;
use Illuminate\Console\OutputStyle;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Support\Facades\Artisan;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Event;
use Illuminate\Support\Facades\Hash;
use Illuminate\Testing\PendingCommand;
use Mockery;
use RuntimeException;
use Symfony\Component\Console\Input\ArrayInput;
use Symfony\Component\Console\Output\BufferedOutput;
use Symfony\Component\Console\Question\Question;
use Tests\TestCase;

class RecoverAdministratorTest extends TestCase
{
    use RefreshDatabase;

    private function administrator(array $attributes = []): User
    {
        return User::factory()->create(array_merge([
            'name' => 'Server Owner',
            'email' => 'owner@example.com',
            'password' => 'correct-horse-battery-staple',
            'is_admin' => true,
            'two_factor_secret' => 'encrypted-totp-secret',
            'two_factor_recovery_codes' => 'encrypted-recovery-codes',
            'two_factor_confirmed_at' => now(),
        ], $attributes));
    }

    private function seedSession(User $administrator): void
    {
        DB::table('sessions')->insert([
            'id' => 'existing-session',
            'user_id' => $administrator->getKey(),
            'ip_address' => '127.0.0.1',
            'user_agent' => 'phpunit',
            'payload' => base64_encode(serialize([])),
            'last_activity' => now()->unix(),
        ]);
    }

    private function recover(array $overrides = []): PendingCommand
    {
        $input = array_replace([
            'email' => 'owner@example.com',
            'confirm' => 'yes',
            'password' => 'a-brand-new-password-1',
        ], $overrides);

        return $this->artisan('hostnur:admin:recover')
            ->expectsQuestion('Existing administrator email', $input['email'])
            ->expectsConfirmation('Reset this administrator password? Multi-factor authentication is preserved.', $input['confirm'])
            ->expectsQuestion('New password (at least 12 characters)', $input['password'])
            ->expectsQuestion('Confirm new password', $input['password_confirmation'] ?? $input['password']);
    }

    public function test_recovery_resets_the_password_and_preserves_mfa_and_setup_state(): void
    {
        $administrator = $this->administrator();
        $this->seedSession($administrator);
        DB::table('panel_setup')->where('id', 1)->update(['administrator_bootstrapped_at' => now()]);
        $marker = DB::table('panel_setup')->where('id', 1)->value('administrator_bootstrapped_at');
        $secret = $administrator->two_factor_secret;

        $this->recover()
            ->expectsOutputToContain('Administrator password reset')
            ->doesntExpectOutputToContain('a-brand-new-password-1')
            ->assertSuccessful();

        $administrator->refresh();

        $this->assertTrue(Hash::check('a-brand-new-password-1', $administrator->password));
        $this->assertSame(1, $administrator->mfa_version);
        $this->assertSame($secret, $administrator->two_factor_secret);
        $this->assertNotNull($administrator->two_factor_confirmed_at);
        $this->assertSame($marker, DB::table('panel_setup')->where('id', 1)->value('administrator_bootstrapped_at'));
        $this->assertDatabaseMissing('sessions', ['user_id' => $administrator->getKey()]);
    }

    public function test_an_unknown_email_is_refused_without_creating_an_account(): void
    {
        $this->administrator();

        $this->artisan('hostnur:admin:recover')
            ->expectsQuestion('Existing administrator email', 'nobody@example.com')
            ->expectsOutputToContain('No administrator matches that email')
            ->assertFailed();

        $this->assertDatabaseCount('users', 1);
        $this->assertSame('owner@example.com', User::query()->sole()->email);
    }

    public function test_an_ordinary_account_is_not_promoted_or_changed(): void
    {
        $user = User::factory()->create([
            'email' => 'member@example.com',
            'password' => 'correct-horse-battery-staple',
            'is_admin' => false,
        ]);
        $hash = $user->password;

        $this->artisan('hostnur:admin:recover')
            ->expectsQuestion('Existing administrator email', 'member@example.com')
            ->expectsOutputToContain('No administrator matches that email')
            ->assertFailed();

        $user->refresh();
        $this->assertFalse($user->is_admin);
        $this->assertSame($hash, $user->password);
    }

    public function test_non_interactive_recovery_is_rejected(): void
    {
        $administrator = $this->administrator();
        $hash = $administrator->password;

        $this->artisan('hostnur:admin:recover', ['--no-interaction' => true])
            ->expectsOutput('Administrator recovery requires an interactive terminal with hidden password input.')
            ->assertFailed();

        $this->assertSame($hash, $administrator->refresh()->password);
    }

    public function test_cancelling_recovery_makes_no_changes(): void
    {
        $administrator = $this->administrator();
        $this->seedSession($administrator);
        $hash = $administrator->password;

        $this->artisan('hostnur:admin:recover')
            ->expectsQuestion('Existing administrator email', 'owner@example.com')
            ->expectsConfirmation('Reset this administrator password? Multi-factor authentication is preserved.', 'no')
            ->expectsOutputToContain('cancelled')
            ->assertFailed();

        $administrator->refresh();
        $this->assertSame($hash, $administrator->password);
        $this->assertSame(0, $administrator->mfa_version);
        $this->assertDatabaseHas('sessions', ['user_id' => $administrator->getKey()]);
    }

    public function test_a_short_or_mismatched_password_is_rejected(): void
    {
        $administrator = $this->administrator();

        $this->recover(['password' => 'too-short', 'password_confirmation' => 'too-short'])
            ->expectsOutputToContain('at least 12 characters')
            ->assertFailed();

        $this->recover(['password' => 'a-brand-new-password-1', 'password_confirmation' => 'a-different-password-1'])
            ->expectsOutputToContain('confirmation does not match')
            ->assertFailed();

        $this->assertTrue(Hash::check('correct-horse-battery-staple', $administrator->refresh()->password));
        $this->assertSame(0, $administrator->mfa_version);
    }

    public function test_a_failed_write_rolls_back_the_password_generation_and_sessions(): void
    {
        $administrator = $this->administrator();
        $this->seedSession($administrator);
        $hash = $administrator->password;

        Event::listen('eloquent.updating: '.User::class, function (): void {
            throw new RuntimeException('internal-recovery-failure');
        });

        $this->recover()
            ->expectsOutput('Administrator recovery failed. No changes were committed. Check database access and hidden terminal input, then retry.')
            ->doesntExpectOutputToContain('internal-recovery-failure')
            ->doesntExpectOutputToContain('a-brand-new-password-1')
            ->assertFailed();

        $administrator->refresh();
        $this->assertSame($hash, $administrator->password);
        $this->assertSame(0, $administrator->mfa_version);
        $this->assertDatabaseHas('sessions', ['user_id' => $administrator->getKey()]);
    }

    public function test_recovery_password_prompts_are_hidden_without_fallback(): void
    {
        $this->administrator();

        $output = new BufferedOutput;
        $style = Mockery::mock(OutputStyle::class.'[askQuestion]', [new ArrayInput([]), $output]);
        $style->shouldReceive('askQuestion')->times(4)->andReturnUsing(function (Question $question): string {
            if (in_array($question->getQuestion(), ['New password (at least 12 characters)', 'Confirm new password'], true)) {
                $this->assertTrue($question->isHidden());
                $this->assertFalse($question->isHiddenFallback());
            }

            return match ($question->getQuestion()) {
                'Existing administrator email' => 'owner@example.com',
                'Reset this administrator password? Multi-factor authentication is preserved.' => 'yes',
                default => 'a-brand-new-password-1',
            };
        });
        $this->app->bind(OutputStyle::class, fn (): OutputStyle => $style);

        $exitCode = Artisan::call('hostnur:admin:recover', [], $output);

        $this->assertSame(0, $exitCode);
        $this->assertTrue(Hash::check('a-brand-new-password-1', User::query()->sole()->password));
    }
}
