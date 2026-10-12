<?php

namespace Tests\Feature;

use App\Http\Middleware\EnsureRecentAuthentication;
use App\Livewire\Panel\ConfirmPassword;
use App\Livewire\Panel\SecuritySettings;
use App\Models\User;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Support\Facades\Cache;
use Illuminate\Support\Facades\Hash;
use Laravel\Fortify\Actions\ConfirmTwoFactorAuthentication;
use Laravel\Fortify\Actions\EnableTwoFactorAuthentication;
use Laravel\Fortify\Fortify;
use Livewire\Livewire;
use PragmaRX\Google2FA\Google2FA;
use Tests\TestCase;

class SecuritySettingsTest extends TestCase
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

        Cache::flush();

        return $administrator;
    }

    private function mfaProof(User $administrator): array
    {
        return ['mfa_verified' => ['id' => $administrator->getKey(), 'version' => $administrator->mfa_version]];
    }

    private function recentAuth(User $administrator, int $ageSeconds = 0, ?int $version = null): array
    {
        return [
            'auth.password_confirmed_at' => now()->unix() - $ageSeconds,
            'auth.password_confirmed_mfa_version' => $version ?? $administrator->mfa_version,
        ];
    }

    private function confirmedSession(User $administrator, int $ageSeconds = 0, ?int $version = null): array
    {
        return array_merge(
            $this->mfaProof($administrator),
            $this->recentAuth($administrator, $ageSeconds, $version),
        );
    }

    public function test_guests_are_redirected_from_security_settings(): void
    {
        $this->get('/panel/security')->assertRedirect(route('login'));
    }

    public function test_a_non_administrator_is_forbidden(): void
    {
        $user = User::factory()->create(['is_admin' => false]);

        $this->actingAs($user)->get('/panel/security')->assertForbidden();
    }

    public function test_security_settings_requires_recent_password_confirmation(): void
    {
        $administrator = $this->confirmedAdministrator();

        $this->actingAs($administrator)
            ->withSession($this->mfaProof($administrator))
            ->get('/panel/security')
            ->assertRedirect(route('password.confirm'));
    }

    public function test_a_recent_confirmation_grants_access(): void
    {
        $administrator = $this->confirmedAdministrator();

        $this->actingAs($administrator)
            ->withSession($this->confirmedSession($administrator))
            ->get('/panel/security')
            ->assertOk()
            ->assertSee('Security settings');
    }

    public function test_an_expired_confirmation_is_rejected(): void
    {
        $administrator = $this->confirmedAdministrator();

        $this->actingAs($administrator)
            ->withSession($this->confirmedSession($administrator, ageSeconds: 301))
            ->get('/panel/security')
            ->assertRedirect(route('password.confirm'));
    }

    public function test_a_confirmation_bound_to_an_old_mfa_generation_is_rejected(): void
    {
        $administrator = $this->confirmedAdministrator();

        $this->actingAs($administrator)
            ->withSession($this->confirmedSession($administrator, version: $administrator->mfa_version - 1))
            ->get('/panel/security')
            ->assertRedirect(route('password.confirm'));
    }

    public function test_the_confirm_password_screen_records_recent_authentication(): void
    {
        $administrator = $this->confirmedAdministrator();

        Livewire::actingAs($administrator)
            ->test(ConfirmPassword::class)
            ->set('password', 'correct-horse-battery')
            ->call('confirm')
            ->assertHasNoErrors()
            ->assertRedirect(route('panel.security'));

        $this->assertSame(
            $administrator->mfa_version,
            session('auth.password_confirmed_mfa_version'),
        );
        $this->assertNotNull(session('auth.password_confirmed_at'));
    }

    public function test_the_confirm_password_screen_rejects_a_wrong_password(): void
    {
        $administrator = $this->confirmedAdministrator();

        Livewire::actingAs($administrator)
            ->test(ConfirmPassword::class)
            ->set('password', 'not-the-password')
            ->call('confirm')
            ->assertHasErrors('password');

        $this->assertNull(session('auth.password_confirmed_at'));
    }

    public function test_mutating_without_recent_authentication_is_blocked(): void
    {
        $administrator = $this->confirmedAdministrator();

        Livewire::actingAs($administrator)
            ->test(SecuritySettings::class)
            ->set('password', 'a-brand-new-password-1')
            ->set('password_confirmation', 'a-brand-new-password-1')
            ->call('changePassword')
            ->assertStatus(423);

        $this->assertTrue(Hash::check('correct-horse-battery', $administrator->refresh()->password));
    }

    public function test_changing_the_password_rotates_the_generation_and_logs_out(): void
    {
        $administrator = $this->confirmedAdministrator();

        $this->actingAs($administrator);
        session($this->recentAuth($administrator));

        Livewire::test(SecuritySettings::class)
            ->set('password', 'a-brand-new-password-1')
            ->set('password_confirmation', 'a-brand-new-password-1')
            ->call('changePassword')
            ->assertHasNoErrors()
            ->assertRedirect(route('login'));

        $administrator->refresh();

        $this->assertTrue(Hash::check('a-brand-new-password-1', $administrator->password));
        $this->assertSame(1, $administrator->mfa_version);
        $this->assertGuest();
    }

    public function test_changing_the_password_validates_the_new_password(): void
    {
        $administrator = $this->confirmedAdministrator();

        $this->actingAs($administrator);
        session($this->recentAuth($administrator));

        Livewire::test(SecuritySettings::class)
            ->set('password', 'short')
            ->set('password_confirmation', 'short')
            ->call('changePassword')
            ->assertHasErrors('password');

        $administrator->refresh();
        $this->assertTrue(Hash::check('correct-horse-battery', $administrator->password));
        $this->assertSame(0, $administrator->mfa_version);
    }

    public function test_disabling_two_factor_clears_it_and_logs_out(): void
    {
        $administrator = $this->confirmedAdministrator();

        $this->actingAs($administrator);
        session($this->recentAuth($administrator));

        Livewire::test(SecuritySettings::class)
            ->call('disableTwoFactor')
            ->assertRedirect(route('login'));

        $administrator->refresh();

        $this->assertNull($administrator->two_factor_secret);
        $this->assertNull($administrator->two_factor_recovery_codes);
        $this->assertNull($administrator->two_factor_confirmed_at);
        $this->assertSame(1, $administrator->mfa_version);
        $this->assertGuest();
    }

    public function test_regenerating_recovery_codes_requires_recent_authentication(): void
    {
        $administrator = $this->confirmedAdministrator();
        $originalCodes = $administrator->recoveryCodes();

        Livewire::actingAs($administrator)
            ->test(SecuritySettings::class)
            ->call('regenerateRecoveryCodes')
            ->assertStatus(423);

        $this->assertSame($originalCodes, $administrator->refresh()->recoveryCodes());
    }

    public function test_regenerating_recovery_codes_replaces_them(): void
    {
        $administrator = $this->confirmedAdministrator();
        $originalCodes = $administrator->recoveryCodes();

        $this->actingAs($administrator);
        session($this->recentAuth($administrator));

        Livewire::test(SecuritySettings::class)
            ->call('regenerateRecoveryCodes')
            ->assertRedirect(route('panel.security'));

        $newCodes = $administrator->refresh()->recoveryCodes();

        $this->assertCount(8, $newCodes);
        $this->assertNotSame($originalCodes, $newCodes);
        $this->assertEmpty(array_intersect($originalCodes, $newCodes));
    }

    public function test_recovery_codes_are_only_rendered_once(): void
    {
        $administrator = $this->confirmedAdministrator();

        $this->actingAs($administrator);
        session()->flash('recovery_codes', ['one-time-code-1', 'one-time-code-2']);

        Livewire::test(SecuritySettings::class)->assertSee('one-time-code-1');
        Livewire::test(SecuritySettings::class)->assertDontSee('one-time-code-1');
    }

    public function test_fortify_mfa_endpoints_require_recent_authentication(): void
    {
        $administrator = $this->confirmedAdministrator();

        $this->actingAs($administrator)
            ->withSession($this->mfaProof($administrator))
            ->delete('/user/two-factor-authentication')
            ->assertRedirect(route('password.confirm'));

        $this->assertNotNull($administrator->refresh()->two_factor_secret);

        $this->actingAs($administrator)
            ->withSession($this->confirmedSession($administrator))
            ->delete('/user/two-factor-authentication')
            ->assertRedirect();

        $this->assertNull($administrator->refresh()->two_factor_secret);
    }

    public function test_recent_authentication_gate_is_registered_as_persistent_livewire_middleware(): void
    {
        $this->assertContains(EnsureRecentAuthentication::class, Livewire::getPersistentMiddleware());
    }
}
