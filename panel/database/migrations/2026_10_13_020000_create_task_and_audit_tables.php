<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::create('tasks', function (Blueprint $table) {
            $table->uuid('id')->primary();
            $table->foreignId('actor_id')->nullable()->constrained('users')->nullOnDelete();
            $table->string('actor_label')->nullable();
            $table->string('operation');
            $table->string('resource_kind')->nullable();
            $table->uuid('resource_id')->nullable();
            $table->string('config_revision')->nullable();
            $table->string('mode');
            $table->string('status');
            $table->uuid('idempotency_key');
            $table->json('parameters');
            $table->json('result')->nullable();
            $table->string('payload_fingerprint', 64);
            $table->uuid('execution_id')->nullable();
            $table->uuid('lease_token')->nullable();
            $table->timestamp('lease_expires_at')->nullable();
            $table->timestamp('requested_at')->nullable();
            $table->timestamp('queued_at')->nullable();
            $table->timestamp('started_at')->nullable();
            $table->timestamp('finished_at')->nullable();
            $table->timestamp('deadline_at')->nullable();
            $table->timestamp('cancel_requested_at')->nullable();
            $table->foreignId('approved_by')->nullable()->constrained('users')->nullOnDelete();
            $table->string('approved_fingerprint', 64)->nullable();
            $table->timestamp('approved_at')->nullable();
            $table->timestamp('approval_expires_at')->nullable();
            $table->unsignedInteger('approved_mfa_version')->nullable();
            $table->string('denied_reason_code')->nullable();
            $table->string('error_code')->nullable();
            $table->text('error_message')->nullable();
            $table->timestamps();

            $table->unique(['actor_id', 'idempotency_key']);
            $table->index('status');
            $table->index(['status', 'lease_expires_at']);
        });

        Schema::create('task_events', function (Blueprint $table) {
            $table->id();
            $table->foreignUuid('task_id')->constrained('tasks')->cascadeOnDelete();
            $table->unsignedInteger('sequence');
            $table->string('type');
            $table->string('level');
            $table->text('message');
            $table->json('metadata')->nullable();
            $table->timestamp('recorded_at');
            $table->unique(['task_id', 'sequence']);
        });

        Schema::create('audit_events', function (Blueprint $table) {
            $table->uuid('id')->primary();
            $table->foreignId('actor_id')->nullable()->constrained('users')->nullOnDelete();
            $table->string('actor_label')->nullable();
            $table->string('action');
            $table->string('outcome');
            $table->string('resource_kind')->nullable();
            $table->uuid('resource_id')->nullable();
            $table->uuid('task_id')->nullable();
            $table->json('metadata')->nullable();
            $table->timestamp('recorded_at');
            $table->index('recorded_at');
            $table->index(['resource_kind', 'resource_id']);
        });
    }

    public function down(): void
    {
        Schema::dropIfExists('audit_events');
        Schema::dropIfExists('task_events');
        Schema::dropIfExists('tasks');
    }
};
