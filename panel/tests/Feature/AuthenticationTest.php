<?php

namespace Tests\Feature;

use App\Models\User;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Tests\TestCase;

class AuthenticationTest extends TestCase
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

    public function test_guests_can_view_the_login_screen(): void
    {
        $this->get('/login')
            ->assertOk()
            ->assertSee('Administrator sign in')
            ->assertSee('name="email"', false)
            ->assertSee('name="password"', false);
    }

    public function test_root_redirects_guests_to_login(): void
    {
        $this->get('/')->assertRedirect(route('login'));
    }

    public function test_root_redirects_authenticated_administrators_to_the_panel(): void
    {
        $this->actingAs($this->administrator())
            ->get('/')
            ->assertRedirect(route('panel.dashboard'));
    }

    public function test_guests_cannot_reach_the_panel(): void
    {
        $this->get('/panel')->assertRedirect(route('login'));
    }

    public function test_an_administrator_can_authenticate_and_reach_the_panel(): void
    {
        $administrator = $this->administrator();

        $this->post('/login', [
            'email' => 'admin@example.com',
            'password' => 'correct-horse-battery',
        ])->assertRedirect(route('panel.dashboard'));

        $this->assertAuthenticatedAs($administrator);

        $this->get('/panel')
            ->assertOk()
            ->assertSee($administrator->name);
    }

    public function test_email_is_trimmed_and_lowercased_during_authentication(): void
    {
        $administrator = $this->administrator(['email' => 'admin@example.com']);

        $this->post('/login', [
            'email' => '  ADMIN@EXAMPLE.COM  ',
            'password' => 'correct-horse-battery',
        ])->assertRedirect(route('panel.dashboard'));

        $this->assertAuthenticatedAs($administrator);
    }

    public function test_an_ordinary_account_cannot_authenticate(): void
    {
        User::factory()->create([
            'email' => 'user@example.com',
            'password' => 'correct-horse-battery',
            'is_admin' => false,
        ]);

        $this->post('/login', [
            'email' => 'user@example.com',
            'password' => 'correct-horse-battery',
        ])->assertSessionHasErrors('email');

        $this->assertGuest();
    }

    public function test_unknown_email_and_wrong_password_share_the_generic_error(): void
    {
        $this->administrator();

        $this->post('/login', ['email' => 'nobody@example.com', 'password' => 'correct-horse-battery'])
            ->assertSessionHasErrors(['email' => trans('auth.failed')]);
        $this->assertGuest();

        $this->post('/login', ['email' => 'admin@example.com', 'password' => 'wrong-password'])
            ->assertSessionHasErrors(['email' => trans('auth.failed')]);
        $this->assertGuest();
    }

    public function test_a_non_administrator_is_forbidden_from_the_panel(): void
    {
        $user = User::factory()->create(['is_admin' => false]);

        $this->actingAs($user)->get('/panel')->assertForbidden();
    }

    public function test_removing_the_administrator_role_takes_effect_immediately(): void
    {
        $administrator = $this->administrator();

        $this->actingAs($administrator)->get('/panel')->assertOk();

        $administrator->is_admin = false;
        $administrator->save();

        $this->actingAs($administrator)->get('/panel')->assertForbidden();
    }

    public function test_logout_invalidates_the_session_and_revokes_panel_access(): void
    {
        $administrator = $this->administrator();

        $this->actingAs($administrator)->get('/panel')->assertOk();

        $this->post('/logout')->assertRedirect(route('login'));

        $this->assertGuest();
        $this->get('/panel')->assertRedirect(route('login'));
    }

    public function test_session_id_rotates_after_password_authentication(): void
    {
        $this->administrator();

        $this->get('/login')->assertOk();
        $before = $this->app['session']->driver()->getId();

        $this->post('/login', [
            'email' => 'admin@example.com',
            'password' => 'correct-horse-battery',
        ])->assertRedirect(route('panel.dashboard'));

        $this->assertNotSame($before, $this->app['session']->driver()->getId());
    }

    public function test_repeated_failed_attempts_are_rate_limited_and_expire(): void
    {
        $this->administrator();

        for ($attempt = 0; $attempt < 5; $attempt++) {
            $this->post('/login', [
                'email' => 'admin@example.com',
                'password' => 'wrong-password',
            ])->assertRedirect();
        }

        $this->post('/login', [
            'email' => 'admin@example.com',
            'password' => 'wrong-password',
        ])->assertStatus(429);

        $this->assertGuest();

        $this->travel(61)->seconds();

        $this->post('/login', [
            'email' => 'admin@example.com',
            'password' => 'correct-horse-battery',
        ])->assertRedirect(route('panel.dashboard'));

        $this->assertAuthenticated();
    }

    public function test_authentication_endpoints_require_a_csrf_token_in_a_production_like_request(): void
    {
        $administrator = $this->administrator();

        $this->app['env'] = 'production';

        $this->post('/login', [
            'email' => 'admin@example.com',
            'password' => 'correct-horse-battery',
        ])->assertStatus(419);

        $this->assertGuest();

        $this->actingAs($administrator);
        $this->post('/logout')->assertStatus(419);
        $this->assertAuthenticatedAs($administrator);
    }
}
