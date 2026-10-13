<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    /**
     * Durable records for the deterministic fake agent. Progress and outcome are
     * derived from the created time + scenario + a controlled clock, so a later
     * process can inspect an execution without an in-memory array or a worker.
     */
    public function up(): void
    {
        Schema::create('fake_agent_executions', function (Blueprint $table) {
            $table->id();
            $table->uuid('execution_id')->unique();
            $table->uuid('request_id')->index();
            $table->uuid('idempotency_key')->unique();
            $table->string('operation');
            $table->string('resource_kind')->nullable();
            $table->uuid('resource_id')->nullable();
            $table->json('parameters');
            $table->string('scenario');
            $table->boolean('read_only')->default(false);
            $table->timestamp('cancel_requested_at')->nullable();
            $table->timestamps();
        });
    }

    public function down(): void
    {
        Schema::dropIfExists('fake_agent_executions');
    }
};
