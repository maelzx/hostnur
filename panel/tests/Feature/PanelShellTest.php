<?php

namespace Tests\Feature;

use App\Models\User;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Support\Facades\Route;
use Tests\Support\InteractsWithPanel;
use Tests\TestCase;

class PanelShellTest extends TestCase
{
    use InteractsWithPanel, RefreshDatabase;

    public function test_guests_are_redirected_from_the_panel(): void
    {
        $this->get('/panel')->assertRedirect(route('login'));
    }

    public function test_a_non_administrator_is_forbidden_from_the_panel(): void
    {
        $user = User::factory()->create(['is_admin' => false]);

        $this->actingAs($user)->get('/panel')->assertForbidden();
    }

    public function test_a_confirmed_administrator_without_proof_is_sent_to_login(): void
    {
        $admin = $this->confirmedAdmin();

        $this->actingAs($admin)->get('/panel')->assertRedirect(route('login'));
    }

    public function test_the_shell_renders_navigation_to_existing_routes(): void
    {
        $admin = $this->confirmedAdmin();

        $response = $this->actingAs($admin)
            ->withSession($this->mfaProof($admin))
            ->get('/panel');

        $response->assertOk()
            ->assertSee('aria-label="Panel"', false)
            ->assertSee('Skip to content')
            ->assertSee('Task history')
            ->assertSee('Audit history')
            ->assertSee('Security settings')
            ->assertSee('Server overview')
            ->assertSee('Scheduled jobs');

        foreach (['panel.dashboard', 'panel.tasks.index', 'panel.audit.index', 'panel.security', 'panel.security.mfa'] as $name) {
            $this->assertTrue(Route::has($name), "Route {$name} is missing.");
            $response->assertSee(route($name), false);
        }
    }

    public function test_panel_links_are_gated_for_an_unconfirmed_administrator(): void
    {
        $admin = User::factory()->create([
            'email' => 'admin@example.com',
            'is_admin' => true,
            'two_factor_secret' => null,
            'two_factor_confirmed_at' => null,
        ]);

        $this->actingAs($admin)
            ->get(route('panel.tasks.index'))
            ->assertRedirect(route('panel.security.mfa'));
    }
}
