<?php

namespace Tests\Feature;

use App\Enums\DomainHttpsPolicy;
use App\Livewire\Panel\ApplicationCreate;
use App\Livewire\Panel\ApplicationShow;
use App\Models\Application;
use App\Services\ApplicationService;
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
            ->set('migration_recovery_ack', true)
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

    public function test_the_wizard_creates_secrets_and_a_schedule(): void
    {
        $admin = $this->confirmedAdmin();
        $this->actingAs($admin)->withSession($this->mfaProof($admin) + $this->recentAuth($admin));

        Livewire::test(ApplicationCreate::class)
            ->set('name', 'Extras App')
            ->set('slug', 'extras-app')
            ->set('secrets', [['name' => 'MAIL_HOST', 'value' => 'smtp.example.com']])
            ->set('schedule_enabled', true)
            ->set('schedule_name', 'nightly')
            ->set('schedule_expression', '0 2 * * *')
            ->set('schedule_command_reference', 'schedule:run')
            ->set('migration_recovery_ack', true)
            ->call('finish')
            ->assertHasNoErrors();

        $application = Application::query()->where('slug', 'extras-app')->firstOrFail();

        $this->assertDatabaseHas('application_secrets', [
            'application_id' => $application->getKey(),
            'name' => 'MAIL_HOST',
        ]);
        $this->assertDatabaseHas('scheduled_jobs', [
            'application_id' => $application->getKey(),
            'name' => 'nightly',
        ]);
    }

    public function test_the_application_page_can_request_a_certificate(): void
    {
        $admin = $this->confirmedAdmin();
        $this->actingAs($admin)->withSession($this->mfaProof($admin) + $this->recentAuth($admin));

        $application = app(ApplicationService::class)->create($admin, [
            'name' => 'Cert App',
            'slug' => 'cert-app',
            'provider' => 'laravel',
            'runtime' => 'php',
            'runtime_version' => '8.3',
        ]);
        $domain = app(ApplicationService::class)->addDomain($admin, $application, 'app.example.com', DomainHttpsPolicy::Auto);

        Livewire::test(ApplicationShow::class, ['application' => $application->getKey()])
            ->call('requestCertificate', $domain->getKey())
            ->assertHasNoErrors();

        $this->assertDatabaseHas('certificates', ['hostname' => 'app.example.com']);
    }
}
