<?php

namespace Tests\Feature;

use App\Livewire\Auth\MfaEnrollment;
use App\Models\User;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Support\Facades\DB;
use Laravel\Fortify\Fortify;
use Livewire\Livewire;
use PragmaRX\Google2FA\Google2FA;
use Tests\TestCase;

class MfaEnrollmentTest extends TestCase
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

    private function currentCode(User $administrator): string
    {
        $secret = Fortify::currentEncrypter()->decrypt($administrator->two_factor_secret);

        return app(Google2FA::class)->getCurrentOtp($secret);
    }

    public function test_guests_are_redirected_from_mfa_settings_to_login(): void
    {
        $this->get('/panel/security/mfa')->assertRedirect(route('login'));
    }

    public function test_non_administrators_are_forbidden_from_mfa_settings(): void
    {
        $user = User::factory()->create(['is_admin' => false]);

        $this->actingAs($user)->get('/panel/security/mfa')->assertForbidden();
    }

    public function test_the_enrollment_component_denies_a_non_administrator(): void
    {
        $user = User::factory()->create(['is_admin' => false]);

        Livewire::actingAs($user)->test(MfaEnrollment::class)->assertStatus(403);
    }

    public function test_an_administrator_can_open_enrollment(): void
    {
        $this->actingAs($this->administrator())
            ->get('/panel/security/mfa')
            ->assertOk()
            ->assertSee('Setup required')
            ->assertSee('Begin setup');
    }

    public function test_enabling_creates_an_encrypted_secret_and_recovery_codes(): void
    {
        $administrator = $this->administrator();

        Livewire::actingAs($administrator)
            ->test(MfaEnrollment::class)
            ->call('enable')
            ->assertHasNoErrors();

        $administrator->refresh();

        $this->assertNotNull($administrator->two_factor_secret);
        $this->assertNotNull($administrator->two_factor_recovery_codes);
        $this->assertNull($administrator->two_factor_confirmed_at);
        $this->assertFalse($administrator->hasEnabledTwoFactorAuthentication());

        $storedSecret = DB::table('users')->where('id', $administrator->id)->value('two_factor_secret');
        $plainSecret = Fortify::currentEncrypter()->decrypt($storedSecret);

        $this->assertNotSame($storedSecret, $plainSecret);
        $this->assertNotEmpty($plainSecret);
        $this->assertCount(8, $administrator->recoveryCodes());
    }

    public function test_the_secret_and_recovery_codes_are_not_serialized(): void
    {
        $administrator = $this->administrator();

        Livewire::actingAs($administrator)->test(MfaEnrollment::class)->call('enable');

        $administrator->refresh();
        $serialized = $administrator->toArray();
        $plainSecret = Fortify::currentEncrypter()->decrypt($administrator->two_factor_secret);

        $this->assertArrayNotHasKey('two_factor_secret', $serialized);
        $this->assertArrayNotHasKey('two_factor_recovery_codes', $serialized);
        $this->assertStringNotContainsString($plainSecret, $administrator->toJson());
    }

    public function test_the_setup_secret_and_recovery_codes_are_shown_only_before_confirmation(): void
    {
        $administrator = $this->administrator();

        Livewire::actingAs($administrator)->test(MfaEnrollment::class)->call('enable');

        $administrator->refresh();
        $plainSecret = Fortify::currentEncrypter()->decrypt($administrator->two_factor_secret);
        $firstRecoveryCode = $administrator->recoveryCodes()[0];

        $this->actingAs($administrator)
            ->get('/panel/security/mfa')
            ->assertOk()
            ->assertSee($plainSecret)
            ->assertSee($firstRecoveryCode);
    }

    public function test_a_valid_code_confirms_enrollment(): void
    {
        $administrator = $this->administrator();

        Livewire::actingAs($administrator)->test(MfaEnrollment::class)->call('enable');
        $administrator->refresh();

        Livewire::actingAs($administrator)
            ->test(MfaEnrollment::class)
            ->set('code', $this->currentCode($administrator))
            ->call('confirm')
            ->assertHasNoErrors();

        $administrator->refresh();

        $this->assertNotNull($administrator->two_factor_confirmed_at);
        $this->assertTrue($administrator->hasEnabledTwoFactorAuthentication());
    }

    public function test_a_wrong_code_does_not_confirm_enrollment(): void
    {
        $administrator = $this->administrator();

        Livewire::actingAs($administrator)->test(MfaEnrollment::class)->call('enable');
        $administrator->refresh();

        $secret = Fortify::currentEncrypter()->decrypt($administrator->two_factor_secret);
        $this->assertFalse(app(Google2FA::class)->verifyKey($secret, '000000'));

        Livewire::actingAs($administrator)
            ->test(MfaEnrollment::class)
            ->set('code', '000000')
            ->call('confirm')
            ->assertHasErrors('code');

        $administrator->refresh();

        $this->assertNull($administrator->two_factor_confirmed_at);
        $this->assertFalse($administrator->hasEnabledTwoFactorAuthentication());
    }

    public function test_a_non_six_digit_code_is_rejected_before_verification(): void
    {
        $administrator = $this->administrator();

        Livewire::actingAs($administrator)->test(MfaEnrollment::class)->call('enable');

        Livewire::actingAs($administrator)
            ->test(MfaEnrollment::class)
            ->set('code', '12345')
            ->call('confirm')
            ->assertHasErrors(['code']);

        $administrator->refresh();
        $this->assertNull($administrator->two_factor_confirmed_at);
    }

    public function test_leading_zeroes_are_preserved_as_a_six_digit_string(): void
    {
        $administrator = $this->administrator();

        Livewire::actingAs($administrator)->test(MfaEnrollment::class)->call('enable');

        Livewire::actingAs($administrator)
            ->test(MfaEnrollment::class)
            ->set('code', '012345')
            ->call('confirm')
            ->assertHasErrors([
                'code' => 'The provided two factor authentication code was invalid.',
            ]);

        $administrator->refresh();
        $this->assertNull($administrator->two_factor_confirmed_at);
    }

    public function test_the_secret_is_hidden_after_confirmation(): void
    {
        $administrator = $this->administrator();

        Livewire::actingAs($administrator)->test(MfaEnrollment::class)->call('enable');
        $administrator->refresh();

        $plainSecret = Fortify::currentEncrypter()->decrypt($administrator->two_factor_secret);
        $firstRecoveryCode = $administrator->recoveryCodes()[0];

        Livewire::actingAs($administrator)
            ->test(MfaEnrollment::class)
            ->set('code', $this->currentCode($administrator))
            ->call('confirm')
            ->assertHasNoErrors();

        $this->actingAs($administrator)
            ->get('/panel/security/mfa')
            ->assertOk()
            ->assertSee('Enabled and confirmed')
            ->assertDontSee($plainSecret)
            ->assertDontSee($firstRecoveryCode);
    }
}
