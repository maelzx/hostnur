<?php

namespace Tests\Unit;

use App\Agent\AgentContractException;
use App\Agent\Dto\CapabilitySnapshot;
use App\Agent\Dto\OperationReceipt;
use App\Agent\Dto\OperationRequest;
use App\Agent\Dto\OperationSnapshot;
use App\Agent\OperationCatalog;
use App\Agent\OperationRequestValidator;
use App\Enums\AgentMode;
use App\Enums\ErrorCode;
use App\Enums\OperationStatus;
use App\Enums\ResourceKind;
use PHPUnit\Framework\Attributes\DataProvider;
use Tests\TestCase;

class AgentContractTest extends TestCase
{
    private function contractsPath(string $path = ''): string
    {
        return rtrim((string) config('hostnur.contracts_path'), '/').'/'.ltrim($path, '/');
    }

    /**
     * @return array<string, mixed>
     */
    private function fixture(string $path): array
    {
        $decoded = json_decode((string) file_get_contents($this->contractsPath($path)), true, flags: JSON_THROW_ON_ERROR);

        $this->assertIsArray($decoded);

        return $decoded;
    }

    private function validator(): OperationRequestValidator
    {
        return app(OperationRequestValidator::class);
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

    public function test_valid_request_fixtures_parse_into_requests(): void
    {
        foreach (glob($this->contractsPath('fixtures/valid/request-*.json')) as $file) {
            $request = $this->validator()->validate($this->fixture('fixtures/valid/'.basename($file)));

            $this->assertInstanceOf(OperationRequest::class, $request);
            $this->assertSame(1, $request->protocolVersion);
        }
    }

    public function test_a_valid_request_carries_its_operation_resource_and_parameters(): void
    {
        $request = $this->validator()->validate(
            $this->fixture('fixtures/valid/request-application-restart.json'),
        );

        $this->assertSame('application.restart', $request->operation);
        $this->assertSame(ResourceKind::Application, $request->resource?->kind);
        $this->assertSame('224065ac-4bc7-4ba4-a06d-8c8bca950c32', $request->resource?->id);
        $this->assertSame([], $request->parameters);
        $this->assertSame(60, $request->timeoutSeconds);
    }

    public function test_a_request_round_trips_to_its_envelope(): void
    {
        $request = $this->validator()->validate(
            $this->fixture('fixtures/valid/request-application-restart.json'),
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

    #[DataProvider('invalidRequests')]
    public function test_invalid_request_fixtures_fail_with_the_expected_error(string $fixture, ErrorCode $code): void
    {
        try {
            $this->validator()->validate($this->fixture('fixtures/invalid/'.$fixture));
            $this->fail("Expected {$fixture} to be rejected.");
        } catch (AgentContractException $exception) {
            $this->assertSame($code, $exception->errorCode);
        }
    }

    /**
     * @return array<string, array{string, ErrorCode}>
     */
    public static function invalidRequests(): array
    {
        return [
            'extra key' => ['request-extra-key.json', ErrorCode::InvalidParameters],
            'unsupported protocol' => ['request-unsupported-protocol.json', ErrorCode::UnsupportedProtocol],
            'unknown operation' => ['request-unknown-operation.json', ErrorCode::UnsupportedOperation],
            'bad uuid' => ['request-bad-uuid.json', ErrorCode::InvalidParameters],
            'wrong type' => ['request-wrong-type.json', ErrorCode::InvalidParameters],
            'unknown parameter' => ['request-unknown-parameter.json', ErrorCode::InvalidParameters],
            'wrong resource kind' => ['request-wrong-resource-kind.json', ErrorCode::InvalidParameters],
            'missing required parameter' => ['request-missing-required-parameter.json', ErrorCode::InvalidParameters],
            'list parameters' => ['request-list-parameters.json', ErrorCode::InvalidParameters],
        ];
    }

    public function test_an_oversized_message_is_rejected(): void
    {
        $payload = $this->fixture('fixtures/valid/request-server-snapshot.json');
        $payload['parameters'] = ['padding' => str_repeat('x', (int) config('hostnur.limits.max_message_bytes') + 1)];

        try {
            $this->validator()->validate($payload);
            $this->fail('Expected the oversized message to be rejected.');
        } catch (AgentContractException $exception) {
            $this->assertSame(ErrorCode::InvalidParameters, $exception->errorCode);
        }
    }

    public function test_valid_response_fixtures_parse_into_dtos(): void
    {
        $receipt = OperationReceipt::fromArray($this->fixture('fixtures/valid/receipt.json'));
        $this->assertSame(OperationStatus::Accepted, $receipt->status);
        $this->assertSame('application.restart', $receipt->operation);
        $this->assertTrue($receipt->status->isActive());

        $running = OperationSnapshot::fromArray($this->fixture('fixtures/valid/snapshot-running.json'));
        $this->assertSame(OperationStatus::Running, $running->status);
        $this->assertSame(40, $running->progress);
        $this->assertCount(2, $running->events);
        $this->assertNull($running->error);

        $failed = OperationSnapshot::fromArray($this->fixture('fixtures/valid/snapshot-failed.json'));
        $this->assertSame(OperationStatus::Failed, $failed->status);
        $this->assertTrue($failed->status->isTerminal());
        $this->assertSame(ErrorCode::OperationFailed, $failed->error?->code);
        $this->assertFalse($failed->error?->retryable);

        $capability = CapabilitySnapshot::fromArray($this->fixture('fixtures/valid/capability-unavailable.json'));
        $this->assertSame(AgentMode::Unavailable, $capability->mode);
        $this->assertFalse($capability->available);
        $this->assertSame([], $capability->operations);
        $this->assertSame('No agent transport is configured.', $capability->reason);
    }

    public function test_an_invalid_response_fixture_is_rejected(): void
    {
        try {
            OperationReceipt::fromArray($this->fixture('fixtures/invalid/receipt-extra-key.json'));
            $this->fail('Expected the receipt fixture to be rejected.');
        } catch (AgentContractException $exception) {
            $this->assertSame(ErrorCode::InvalidParameters, $exception->errorCode);
        }
    }
}
