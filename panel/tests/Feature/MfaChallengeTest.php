<?php

namespace Tests\Feature;

use App\Http\Middleware\EnsureAdministrator;
use App\Http\Middleware\EnsureMfaVerified;
use App\Models\User;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Support\Facades\Cache;
use Laravel\Fortify\Actions\ConfirmTwoFactorAuthentication;
use Laravel\Fortify\Actions\EnableTwoFactorAuthentication;
use Laravel\Fortify\Fortify;
use Livewire\Livewire;
use PragmaRX\Google2FA\Google2FA;
use Tests\TestCase;

class MfaChallengeTest extends TestCase
{
    use RefreshDatabase;

    private function administrator(array $attributes = []): User
    {
        return User::factory()->create(array_merge([
            'email' => 'admin@example.com',
            'password' => 'correct-horse-battery',
            'is_admin' => true,
        ], $attributes));
    }

    private function confirmedAdministrator(): User
    {
        $administrator = $this->administrator();

        app(EnableTwoFactorAuthentication::class)($administrator);
        $administrator->refresh();

        $code = app(Google2FA::class)->getCurrentOtp(
            Fortify::currentEncrypter()->decrypt($administrator->two_factor_secret)
        );

        app(ConfirmTwoFactorAuthentication::class)($administrator, $code);
        $administrator->refresh();

        // Clear Fortify's replay cache so a challenge code in the same time
        // window as the confirmation code is not treated as a replay.
        Cache::flush();

        return $administrator;
    }

    private function currentCode(User $administrator): string
    {
        return app(Google2FA::class)->getCurrentOtp(
            Fortify::currentEncrypter()->decrypt($administrator->two_factor_secret)
        );
    }

    private function proofFor(User $administrator): array
    {
        return ['id' => $administrator->getKey(), 'version' => $administrator->mfa_version];
    }

    public function test_a_confirmed_administrator_is_sent_to_the_challenge_after_password(): void
    {
        $administrator = $this->confirmedAdministrator();

        $this->post('/login', [
            'email' => 'admin@example.com',
            'password' => 'correct-horse-battery',
        ])->assertRedirect(route('two-factor.login'));

        $this->assertGuest();
        $this->assertSame($administrator->getKey(), session('login.id'));
    }

    public function test_password_only_does_not_reach_the_panel(): void
    {
        $this->confirmedAdministrator();

        $this->post('/login', [
            'email' => 'admin@example.com',
            'password' => 'correct-horse-battery',
        ]);

        $this->assertGuest();
        $this->get('/panel')->assertRedirect(route('login'));
    }

    public function test_the_challenge_screen_requires_a_started_challenge(): void
    {
        $this->get('/two-factor-challenge')->assertRedirect(route('login'));
    }

    public function test_the_challenge_screen_is_shown_for_a_started_challenge(): void
    {
        $this->confirmedAdministrator();

        $this->post('/login', [
            'email' => 'admin@example.com',
            'password' => 'correct-horse-battery',
        ]);

        $this->get('/two-factor-challenge')
            ->assertOk()
            ->assertSee('Two-factor verification')
            ->assertSee('name="code"', false)
            ->assertSee('name="recovery_code"', false);
    }

    public function test_a_valid_code_completes_login_and_grants_panel_access(): void
    {
        $administrator = $this->confirmedAdministrator();

        $this->post('/login', [
            'email' => 'admin@example.com',
            'password' => 'correct-horse-battery',
        ]);

        $this->post('/two-factor-challenge', [
            'code' => $this->currentCode($administrator),
        ])->assertRedirect(route('panel.dashboard'));

        $this->assertAuthenticatedAs($administrator);
        $this->assertSame(
            $administrator->mfa_version,
            session('mfa_verified')['version'] ?? null,
        );
        $this->get('/panel')->assertOk()->assertSee($administrator->name);
    }

    public function test_an_invalid_code_is_rejected(): void
    {
        $administrator = $this->confirmedAdministrator();

        $secret = Fortify::currentEncrypter()->decrypt($administrator->two_factor_secret);
        $this->assertFalse(app(Google2FA::class)->verifyKey($secret, '000000'));

        $this->post('/login', [
            'email' => 'admin@example.com',
            'password' => 'correct-horse-battery',
        ]);

        $this->post('/two-factor-challenge', ['code' => '000000'])
            ->assertRedirect(route('two-factor.login'))
            ->assertSessionHasErrors('code');

        $this->assertGuest();
        $this->get('/panel')->assertRedirect(route('login'));
    }

    public function test_a_challenge_without_a_started_login_is_rejected(): void
    {
        $this->post('/two-factor-challenge', ['code' => '123456'])
            ->assertRedirect(route('two-factor.login'));

        $this->assertGuest();
    }

    public function test_a_recovery_code_completes_login_and_cannot_be_reused(): void
    {
        $administrator = $this->confirmedAdministrator();
        $recoveryCode = $administrator->recoveryCodes()[0];

        $this->post('/login', [
            'email' => 'admin@example.com',
            'password' => 'correct-horse-battery',
        ]);

        $this->post('/two-factor-challenge', ['recovery_code' => $recoveryCode])
            ->assertRedirect(route('panel.dashboard'));

        $this->assertAuthenticatedAs($administrator);
        $this->assertNotContains($recoveryCode, $administrator->refresh()->recoveryCodes());

        $this->post('/logout');

        $this->post('/login', [
            'email' => 'admin@example.com',
            'password' => 'correct-horse-battery',
        ]);

        $this->post('/two-factor-challenge', ['recovery_code' => $recoveryCode])
            ->assertRedirect(route('two-factor.login'))
            ->assertSessionHasErrors('recovery_code');

        $this->assertGuest();
    }

    public function test_an_unconfirmed_administrator_is_sent_to_enrollment(): void
    {
        $administrator = $this->administrator();

        $this->actingAs($administrator)
            ->get('/panel')
            ->assertRedirect(route('panel.security.mfa'));
    }

    public function test_a_confirmed_administrator_without_proof_is_logged_out(): void
    {
        $administrator = $this->confirmedAdministrator();

        $this->actingAs($administrator)
            ->get('/panel')
            ->assertRedirect(route('login'));

        $this->assertGuest();
    }

    public function test_a_valid_proof_grants_panel_access(): void
    {
        $administrator = $this->confirmedAdministrator();

        $this->actingAs($administrator)
            ->withSession(['mfa_verified' => $this->proofFor($administrator)])
            ->get('/panel')
            ->assertOk();
    }

    public function test_a_stale_proof_is_rejected_after_the_generation_changes(): void
    {
        $administrator = $this->confirmedAdministrator();

        $staleProof = $this->proofFor($administrator);

        $administrator->mfa_version = 1;
        $administrator->save();

        $this->actingAs($administrator)
            ->withSession(['mfa_verified' => $staleProof])
            ->get('/panel')
            ->assertRedirect(route('login'));

        $this->assertGuest();
    }

    public function test_hostnur_gates_are_registered_as_persistent_livewire_middleware(): void
    {
        $persistent = Livewire::getPersistentMiddleware();

        $this->assertContains(EnsureAdministrator::class, $persistent);
        $this->assertContains(EnsureMfaVerified::class, $persistent);
    }
}
