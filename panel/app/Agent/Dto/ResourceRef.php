<?php

namespace App\Agent\Dto;

use App\Agent\AgentContractException;
use App\Enums\ResourceKind;
use Illuminate\Support\Str;

final readonly class ResourceRef
{
    public function __construct(
        public ResourceKind $kind,
        public string $id,
    ) {}

    /**
     * @param  array<string, mixed>  $data
     */
    public static function fromArray(array $data): self
    {
        $extra = array_diff(array_keys($data), ['kind', 'id']);

        if ($extra !== []) {
            throw AgentContractException::invalidParameters(
                'Resource has unexpected keys: '.implode(', ', $extra).'.'
            );
        }

        $kind = $data['kind'] ?? null;
        $kind = is_string($kind) ? ResourceKind::tryFrom($kind) : null;

        if ($kind === null) {
            throw AgentContractException::invalidParameters('Resource kind is unknown.');
        }

        $id = $data['id'] ?? null;

        if (! is_string($id) || ! Str::isUuid($id)) {
            throw AgentContractException::invalidParameters('Resource id must be a UUID.');
        }

        return new self($kind, $id);
    }

    /**
     * @return array{kind: string, id: string}
     */
    public function toArray(): array
    {
        return ['kind' => $this->kind->value, 'id' => $this->id];
    }
}
