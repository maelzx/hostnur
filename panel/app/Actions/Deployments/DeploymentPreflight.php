<?php

namespace App\Actions\Deployments;

use App\Agent\OperationCatalog;
use App\Contracts\AgentClient;
use App\Models\Application;

/**
 * Validates panel configuration only. It never contacts the repository or an
 * agent host, and it reports what remains agent-dependent.
 */
final class DeploymentPreflight
{
    public function __construct(
        private readonly OperationCatalog $catalog,
        private readonly AgentClient $agent,
    ) {}

    /**
     * @return list<array{key: string, label: string, required: bool, passed: bool, message: string|null}>
     */
    public function deployChecks(Application $application): array
    {
        $settings = $application->revisions()->first()?->settings ?? [];

        $needsCredential = is_string($application->repository_url)
            && (str_starts_with($application->repository_url, 'ssh://') || str_starts_with($application->repository_url, 'git@'));

        return [
            $this->check('not_archived', 'Application is not archived', true, ! $application->isArchived(), 'Archived applications cannot be deployed.'),
            $this->check('provider', 'Provider is supported', true, in_array($application->provider, config('hostnur.applications.providers'), true), 'Unsupported provider.'),
            $this->check('runtime', 'Runtime is supported', true, in_array($application->runtime, config('hostnur.applications.runtimes'), true), 'Unsupported runtime.'),
            $this->check('repository', 'Repository URL is configured', true, is_string($application->repository_url) && $application->repository_url !== '', 'Configure a repository URL first.'),
            $this->check('ref', 'Git ref is configured', true, is_string($application->git_ref) && $application->git_ref !== '', 'Configure a branch, tag or commit first.'),
            $this->check('credential', 'Source credential present when required', true, ! $needsCredential || $application->source_credential_id !== null, 'SSH sources require a stored credential.'),
            $this->check('health', 'Health check path is configured', true, ! empty($settings['health_path']), 'Set a health check path in the application configuration.'),
            $this->capability('application.deploy'),
            $this->check('domains', 'Domains configured', false, true, $application->domains()->exists() ? null : 'No domains configured; add a domain before the application can be reached.'),
        ];
    }

    /**
     * @return list<array{key: string, label: string, required: bool, passed: bool, message: string|null}>
     */
    public function operationChecks(Application $application, string $operation): array
    {
        return [
            $this->check('not_archived', 'Application is not archived', true, ! $application->isArchived(), 'Archived applications cannot be operated on.'),
            $this->capability($operation),
        ];
    }

    /**
     * @param  list<array{required: bool, passed: bool}>  $checks
     */
    public function passes(array $checks): bool
    {
        foreach ($checks as $check) {
            if ($check['required'] && ! $check['passed']) {
                return false;
            }
        }

        return true;
    }

    /**
     * @param  list<array{key: string, label: string, required: bool, passed: bool, message: string|null}>  $checks
     * @return list<array{key: string, label: string, required: bool, passed: bool, message: string|null}>
     */
    public function failures(array $checks): array
    {
        return array_values(array_filter(
            $checks,
            fn (array $check): bool => $check['required'] && ! $check['passed'],
        ));
    }

    /**
     * @return array{key: string, label: string, required: bool, passed: bool, message: string|null}
     */
    private function capability(string $operation): array
    {
        $capabilities = $this->agent->capabilities();
        $supported = $this->catalog->supports($operation)
            && $capabilities->available
            && in_array($operation, $capabilities->operations, true);

        return $this->check(
            'capability',
            'Agent supports this operation',
            true,
            $supported,
            $capabilities->reason ?? 'The agent cannot run this operation yet.',
        );
    }

    /**
     * @return array{key: string, label: string, required: bool, passed: bool, message: string|null}
     */
    private function check(string $key, string $label, bool $required, bool $passed, ?string $message): array
    {
        return [
            'key' => $key,
            'label' => $label,
            'required' => $required,
            'passed' => $passed,
            'message' => $passed ? null : $message,
        ];
    }
}
