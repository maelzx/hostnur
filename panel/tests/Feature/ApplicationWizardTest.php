<?php

namespace Tests\Feature;

use App\Livewire\Panel\ApplicationCreate;
use App\Models\Application;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Livewire\Livewire;
use Tests\Support\InteractsWithPanel;
use Tests\TestCase;

class ApplicationWizardTest extends TestCase
{
    use InteractsWithPanel, RefreshDatabase;

    public function test_the_create_route_is_gated_to_guests(): void
    {
        $this->get('/panel/applications/create')->assertRedirect(route('login'));
    }

    public function test_the_wizard_requires_the_basics_before_advancing(): void
    {
        $admin = $this->confirmedAdmin();
        $this->actingAs($admin)->withSession($this->mfaProof($admin) + $this->recentAuth($admin));

        Livewire::test(ApplicationCreate::class)
            ->call('next')
            ->assertHasErrors(['name', 'slug'])
            ->assertSet('step', 0);
    }

    public function test_the_wizard_creates_application_source_domain_certificate_and_database(): void
    {
        $admin = $this->confirmedAdmin();
        $this->actingAs($admin)->withSession($this->mfaProof($admin) + $this->recentAuth($admin));

        Livewire::test(ApplicationCreate::class)
            ->set('name', 'Wizard App')
            ->set('slug', 'wizard-app')
            ->set('provider', 'laravel')
            ->set('runtime', 'php')
            ->set('runtime_version', '8.3')
            ->set('repository_url', 'https://github.com/org/repo.git')
            ->set('git_ref', 'main')
            ->set('hostname', 'wizard.example.com')
            ->set('request_certificate', true)
            ->set('provision_database', true)
            ->set('engine', 'mariadb')
            ->set('database_name', 'wizard_db')
            ->set('database_username', 'wizard_user')
            ->call('finish')
            ->assertHasNoErrors();

        $application = Application::query()->where('slug', 'wizard-app')->firstOrFail();

        $this->assertSame('https://github.com/org/repo.git', $application->repository_url);
        $this->assertDatabaseHas('application_domains', [
            'application_id' => $application->getKey(),
            'hostname' => 'wizard.example.com',
        ]);
        $this->assertDatabaseHas('certificates', ['hostname' => 'wizard.example.com']);
        $this->assertDatabaseHas('database_requests', [
            'application_id' => $application->getKey(),
            'database_name' => 'wizard_db',
        ]);
    }
}
