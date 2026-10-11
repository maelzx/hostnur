<?php

namespace Tests\Feature;

use App\Models\User;
use Closure;
use Illuminate\Console\OutputStyle;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Support\Facades\Artisan;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Event;
use Illuminate\Support\Facades\Hash;
use Illuminate\Support\Facades\Schema;
use Illuminate\Testing\PendingCommand;
use Mockery;
use PHPUnit\Framework\Attributes\DataProvider;
use RuntimeException;
use Symfony\Component\Console\Input\ArrayInput;
use Symfony\Component\Console\Output\BufferedOutput;
use Symfony\Component\Console\Question\Question;
use Tests\TestCase;

class BootstrapAdministratorTest extends TestCase
{
    use RefreshDatabase;

    public function test_it_creates_one_administrator_with_a_hashed_password_and_closes_setup(): void
    {
        $password = str_repeat('a', 72);

        $this->bootstrap(['name' => '  Server Owner  ', 'email' => '  OWNER@EXAMPLE.COM  ', 'password' => $password])
            ->expectsOutput('Hostnur administrator created. Setup is now closed.')
            ->doesntExpectOutputToContain($password)
            ->assertSuccessful();

        $administrator = User::query()->sole();

        $this->assertSame('Server Owner', $administrator->name);
        $this->assertSame('owner@example.com', $administrator->email);
        $this->assertTrue($administrator->is_admin);
        $this->assertNull($administrator->email_verified_at);
        $this->assertNotSame($password, $administrator->password);
        $this->assertTrue(Hash::check($password, $administrator->password));
        $this->assertNotNull($this->completedAt());
    }

    #[DataProvider('invalidCredentials')]
    public function test_invalid_input_does_not_create_an_account_or_close_setup(array $input, string $error): void
    {
        $this->bootstrap($input)
            ->expectsOutputToContain($error)
            ->assertFailed();

        $this->assertDatabaseCount('users', 0);
        $this->assertNull($this->completedAt());
    }

    public static function invalidCredentials(): array
    {
        return [
            'blank name' => [['name' => '  '], 'The name field is required.'],
            'blank email' => [['email' => ''], 'The email field is required.'],
            'invalid email' => [['email' => 'not-an-email'], 'The email field must be a valid email address.'],
            'short password' => [['password' => 'too-short'], 'The password field must be at least 12 characters.'],
            'missing password' => [['password' => ''], 'The password field is required.'],
            'mismatched confirmation' => [['password_confirmation' => 'different-secret'], 'The password field confirmation does not match.'],
            'over bcrypt byte limit' => [['password' => str_repeat('a', 73)], 'The password must not exceed 72 bytes.'],
            'unicode over byte limit' => [['password' => str_repeat('é', 37)], 'The password must not exceed 72 bytes.'],
            'null byte' => [['password' => "long-passphrase\0"], 'The password must not contain null bytes.'],
        ];
    }

    public function test_setup_remains_closed_after_the_administrator_is_deleted(): void
    {
        $this->bootstrap()->assertSuccessful();
        $completedAt = $this->completedAt();
        User::query()->delete();

        $this->artisan('hostnur:admin:bootstrap')
            ->expectsOutput('Administrator setup has already been completed.')
            ->assertFailed();

        $this->assertDatabaseCount('users', 0);
        $this->assertSame($completedAt, $this->completedAt());
    }

    public function test_existing_accounts_are_not_promoted_or_overwritten(): void
    {
        $user = User::factory()->create();
        $passwordHash = $user->password;

        $this->artisan('hostnur:admin:bootstrap')
            ->expectsOutput('Accounts already exist. Bootstrap cannot create another account.')
            ->assertFailed();

        $this->assertDatabaseCount('users', 1);
        $this->assertFalse($user->refresh()->is_admin);
        $this->assertSame($passwordHash, $user->password);
        $this->assertNull($this->completedAt());
    }

    public function test_non_interactive_creation_is_rejected_without_changes(): void
    {
        $this->artisan('hostnur:admin:bootstrap', ['--no-interaction' => true])
            ->expectsOutput('Administrator setup requires an interactive terminal with hidden password input.')
            ->assertFailed();

        $this->assertDatabaseCount('users', 0);
        $this->assertNull($this->completedAt());
    }

    public function test_missing_migrations_produce_an_actionable_error(): void
    {
        Schema::drop('panel_setup');

        $this->artisan('hostnur:admin:bootstrap')
            ->expectsOutput('Run php artisan migrate before administrator setup.')
            ->assertFailed();

        $this->assertDatabaseCount('users', 0);
    }

    public function test_missing_setup_state_is_not_silently_recreated(): void
    {
        DB::table('panel_setup')->delete();

        $this->artisan('hostnur:admin:bootstrap')
            ->expectsOutput('Panel setup state is missing. Restore it before retrying.')
            ->assertFailed();

        $this->assertDatabaseCount('users', 0);
        $this->assertDatabaseCount('panel_setup', 0);
    }

    public function test_account_creation_failure_rolls_back_setup_without_leaking_secrets(): void
    {
        $password = 'private-recovery-passphrase';
        Event::listen('eloquent.creating: '.User::class, function () use ($password): void {
            throw new RuntimeException('internal-failure-'.$password);
        });

        $this->bootstrap(['password' => $password])
            ->expectsOutput('Administrator setup failed. No changes were committed. Check database access and hidden terminal input, then retry.')
            ->doesntExpectOutputToContain($password)
            ->doesntExpectOutputToContain('internal-failure')
            ->assertFailed();

        $this->assertDatabaseCount('users', 0);
        $this->assertNull($this->completedAt());
    }

    public function test_another_setup_completing_during_prompts_cannot_create_a_second_administrator(): void
    {
        $result = $this->runWithInterleavedWrite(function (): void {
            DB::transaction(function (): void {
                DB::table('panel_setup')->where('id', 1)->update(['administrator_bootstrapped_at' => now()]);
                User::factory()->create(['email' => 'winner@example.com', 'is_admin' => true]);
            });
        });

        $this->assertSame(1, $result['exit_code']);
        $this->assertStringContainsString('Administrator setup has already been completed', $result['output']);
        $this->assertDatabaseCount('users', 1);
        $this->assertSame('winner@example.com', User::query()->sole()->email);
        $this->assertNotNull($this->completedAt());
    }

    public function test_cancelled_account_creation_does_not_report_success_or_close_setup(): void
    {
        Event::listen('eloquent.creating: '.User::class, fn (): bool => false);

        $this->bootstrap()
            ->expectsOutputToContain('Administrator setup failed. No changes were committed.')
            ->doesntExpectOutputToContain('Hostnur administrator created')
            ->assertFailed();

        $this->assertDatabaseCount('users', 0);
        $this->assertNull($this->completedAt());
    }

    public function test_the_administrator_flag_cannot_be_mass_assigned(): void
    {
        $user = User::query()->create([
            'name' => 'Ordinary Account',
            'email' => 'ordinary@example.com',
            'password' => 'correct-horse-battery-staple',
            'is_admin' => true,
        ]);

        $this->assertFalse($user->refresh()->is_admin);
    }

    public function test_an_account_appearing_during_prompts_is_preserved_and_the_claim_is_rolled_back(): void
    {
        $result = $this->runWithInterleavedWrite(function (): void {
            User::factory()->create(['email' => 'existing@example.com']);
        });

        $this->assertSame(1, $result['exit_code']);
        $this->assertStringContainsString('Accounts already exist', $result['output']);
        $this->assertDatabaseCount('users', 1);
        $this->assertSame('existing@example.com', User::query()->sole()->email);
        $this->assertFalse(User::query()->sole()->is_admin);
        $this->assertNull($this->completedAt());
    }

    private function bootstrap(array $overrides = []): PendingCommand
    {
        $input = array_replace([
            'name' => 'Server Owner',
            'email' => 'owner@example.com',
            'password' => 'correct-horse-battery-staple',
        ], $overrides);

        return $this->artisan('hostnur:admin:bootstrap')
            ->expectsQuestion('Administrator name', $input['name'])
            ->expectsQuestion('Administrator email', $input['email'])
            ->expectsQuestion('Password (at least 12 characters)', $input['password'])
            ->expectsQuestion('Confirm password', $input['password_confirmation'] ?? $input['password']);
    }

    private function completedAt(): ?string
    {
        return DB::table('panel_setup')->where('id', 1)->value('administrator_bootstrapped_at');
    }

    /**
     * @return array{exit_code: int, output: string}
     */
    private function runWithInterleavedWrite(Closure $write): array
    {
        $output = new BufferedOutput;
        $style = Mockery::mock(OutputStyle::class.'[askQuestion]', [new ArrayInput([]), $output]);
        $style->shouldReceive('askQuestion')->times(4)->andReturnUsing(function (Question $question) use ($write): string {
            $answers = [
                'Administrator name' => 'Losing Owner',
                'Administrator email' => 'loser@example.com',
                'Password (at least 12 characters)' => 'correct-horse-battery-staple',
                'Confirm password' => 'correct-horse-battery-staple',
            ];

            if (str_contains($question->getQuestion(), 'password') || str_contains($question->getQuestion(), 'Password')) {
                $this->assertTrue($question->isHidden());
                $this->assertFalse($question->isHiddenFallback());
            }

            if ($question->getQuestion() === 'Confirm password') {
                $write();
            }

            return $answers[$question->getQuestion()];
        });
        $this->app->bind(OutputStyle::class, fn (): OutputStyle => $style);

        $exitCode = Artisan::call('hostnur:admin:bootstrap', [], $output);

        return ['exit_code' => $exitCode, 'output' => $output->fetch()];
    }
}
