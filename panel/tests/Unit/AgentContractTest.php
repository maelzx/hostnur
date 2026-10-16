<?php

namespace Tests\Unit;

use App\Agent\AgentContractException;
use App\Agent\Dto\ApplicationLogSnapshot;
use App\Agent\Dto\ApplicationMetricsSnapshot;
use App\Agent\Dto\CancellationReceipt;
use App\Agent\Dto\CapabilitySnapshot;
use App\Agent\Dto\OperationReceipt;
use App\Agent\Dto\OperationRequest;
use App\Agent\Dto\OperationSnapshot;
use App\Agent\Dto\ServerSnapshot;
use App\Agent\OperationCatalog;
use App\Agent\OperationRequestValidator;
use App\Enums\AgentMode;
use App\Enums\CancellationStatus;
use App\Enums\ErrorCode;
use App\Enums\OperationStatus;
use Tests\Support\ContractFixtures;
use Tests\Support\JsonSchemaValidator;
use Tests\TestCase;

class AgentContractTest extends TestCase
{
    private function validator(): OperationRequestValidator
    {
        return app(OperationRequestValidator::class);
    }

    /**
     * @param  array<string, mixed>  $payload
     */
    private function parseResponse(string $fixture, array $payload): object
    {
        return match (true) {
            str_starts_with($fixture, 'receipt') => OperationReceipt::fromArray($payload),
            str_starts_with($fixture, 'application-log') => ApplicationLogSnapshot::fromArray($payload),
            str_starts_with($fixture, 'application-metrics') => ApplicationMetricsSnapshot::fromArray($payload),
            str_starts_with($fixture, 'server-snapshot') => ServerSnapshot::fromArray($payload),
            str_starts_with($fixture, 'snapshot') => OperationSnapshot::fromArray($payload),
            str_starts_with($fixture, 'capability') => CapabilitySnapshot::fromArray($payload),
            str_starts_with($fixture, 'cancellation') => CancellationReceipt::fromArray($payload),
            default => $this->fail("No DTO mapping for fixture {$fixture}."),
        };
    }

    public function test_the_catalog_lists_protocol_versions_and_operations(): void
    {
        $catalog = app(OperationCatalog::class);

        $this->assertSame([1], $catalog->protocolVersions());
        $this->assertTrue($catalog->supports('application.restart'));
        $this->assertFalse($catalog->supports('server.obliterate'));
        $this->assertContains('server.snapshot', $catalog->names());
        $this->assertSame('application', $catalog->definition('application.deploy')['resource']);
    }

    public function test_every_valid_fixture_matches_its_json_schema(): void
    {
        $fixtures = array_merge(ContractFixtures::validFixtures(), ContractFixtures::fakeFixtures());

        $this->assertNotEmpty($fixtures);

        foreach ($fixtures as $fixture => $payload) {
            JsonSchemaValidator::assertValid(ContractFixtures::schema($fixture), $payload, $fixture);
        }
    }

    public function test_valid_request_fixtures_parse_into_requests(): void
    {
        $this->assertNotEmpty(ContractFixtures::validRequests());

        foreach (ContractFixtures::validRequests() as $fixture => $payload) {
            $request = $this->validator()->validate($payload);

            $this->assertInstanceOf(OperationRequest::class, $request, $fixture);
            $this->assertSame(1, $request->protocolVersion, $fixture);
        }
    }

    public function test_valid_response_fixtures_parse_into_dtos(): void
    {
        $timestamp = '2026-10-12T12:00:00Z';

        $receipt = OperationReceipt::fromArray(ContractFixtures::json('fixtures/valid/receipt.json'));
        $this->assertSame(OperationStatus::Accepted, $receipt->status);
        $this->assertSame('application.restart', $receipt->operation);
        $this->assertTrue($receipt->status->isActive());

        $running = OperationSnapshot::fromArray(ContractFixtures::json('fixtures/valid/snapshot-running.json'));
        $this->assertSame(OperationStatus::Running, $running->status);
        $this->assertSame(40, $running->progress);
        $this->assertCount(2, $running->events);
        $this->assertNull($running->error);

        $failed = OperationSnapshot::fromArray(ContractFixtures::json('fixtures/valid/snapshot-failed.json'));
        $this->assertSame(OperationStatus::Failed, $failed->status);
        $this->assertTrue($failed->status->isTerminal());
        $this->assertSame(ErrorCode::OperationFailed, $failed->error?->code);
        $this->assertFalse($failed->error?->retryable);

        $capability = CapabilitySnapshot::fromArray(ContractFixtures::json('fixtures/valid/capability-unavailable.json'));
        $this->assertSame(AgentMode::Unavailable, $capability->mode);
        $this->assertFalse($capability->available);
        $this->assertSame([], $capability->operations);
        $this->assertSame('No agent transport is configured.', $capability->reason);

        $server = ServerSnapshot::fromArray(ContractFixtures::json('fixtures/valid/server-snapshot-unavailable.json'));
        $this->assertSame(AgentMode::Unavailable, $server->mode);
        $this->assertFalse($server->available);
        $this->assertSame([], $server->metrics);
        $this->assertSame([], $server->services);

        $cancellation = CancellationReceipt::fromArray(ContractFixtures::json('fixtures/valid/cancellation-receipt.json'));
        $this->assertSame(CancellationStatus::Requested, $cancellation->status);
        $this->assertSame('d1e2f3a4-b5c6-4d7e-8f9a-0b1c2d3e4f5a', $cancellation->executionId);

        $this->assertSame($timestamp, $receipt->acceptedAt);
    }

    public function test_a_valid_request_round_trips_to_its_envelope(): void
    {
        $request = $this->validator()->validate(
            ContractFixtures::json('fixtures/valid/request-application-restart.json'),
        );

        $envelope = $request->toArray();

        $this->assertSame(1, $envelope['protocol_version']);
        $this->assertSame('application.restart', $envelope['operation']);
        $this->assertSame([
            'kind' => 'application',
            'id' => '224065ac-4bc7-4ba4-a06d-8c8bca950c32',
        ], $envelope['resource']);
        $this->assertSame('{}', json_encode($envelope['parameters']));
    }

    public function test_invalid_request_fixtures_fail_with_the_expected_error(): void
    {
        $fixtures = ContractFixtures::invalidRequests();

        $this->assertNotEmpty($fixtures);

        foreach ($fixtures as $fixture => $entry) {
            try {
                $this->validator()->validate($entry['payload']);
                $this->fail("Expected {$fixture} to be rejected.");
            } catch (AgentContractException $exception) {
                $this->assertSame(ErrorCode::from($entry['error']), $exception->errorCode, $fixture);
            }
        }
    }

    public function test_invalid_response_fixtures_are_rejected(): void
    {
        $fixtures = ContractFixtures::invalidResponses();

        $this->assertNotEmpty($fixtures);

        foreach ($fixtures as $fixture => $entry) {
            try {
                $this->parseResponse($fixture, $entry['payload']);
                $this->fail("Expected {$fixture} to be rejected.");
            } catch (AgentContractException $exception) {
                $this->assertSame(ErrorCode::from($entry['error']), $exception->errorCode, $fixture);
            }
        }
    }

    public function test_an_oversized_message_is_rejected(): void
    {
        $payload = ContractFixtures::json('fixtures/valid/request-server-snapshot.json');
        $payload['parameters'] = ['padding' => str_repeat('x', (int) config('hostnur.limits.max_message_bytes') + 1)];

        try {
            $this->validator()->validate($payload);
            $this->fail('Expected the oversized message to be rejected.');
        } catch (AgentContractException $exception) {
            $this->assertSame(ErrorCode::InvalidParameters, $exception->errorCode);
        }
    }
}
