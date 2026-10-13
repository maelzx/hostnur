<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::create('deployments', function (Blueprint $table) {
            $table->uuid('id')->primary();
            $table->foreignUuid('application_id')->constrained('applications')->cascadeOnDelete();
            $table->uuid('revision_id')->nullable();
            $table->unsignedInteger('revision_number')->nullable();
            $table->uuid('task_id')->nullable();
            $table->string('operation');
            $table->string('git_ref')->nullable();
            $table->string('resolved_commit')->nullable();
            $table->string('runtime');
            $table->string('mode');
            $table->string('state')->default('requested');
            $table->boolean('run_migrations')->default(false);
            $table->timestamp('migration_acknowledged_at')->nullable();
            $table->string('release_reference')->nullable();
            $table->string('previous_release_reference')->nullable();
            $table->json('health_summary')->nullable();
            $table->string('error_code')->nullable();
            $table->text('error_message')->nullable();
            $table->uuid('idempotency_key')->unique();
            $table->timestamp('requested_at')->nullable();
            $table->timestamp('started_at')->nullable();
            $table->timestamp('finished_at')->nullable();
            $table->foreignId('created_by')->nullable()->constrained('users')->nullOnDelete();
            $table->timestamps();

            $table->foreign('revision_id')->references('id')->on('application_revisions')->nullOnDelete();
            $table->foreign('task_id')->references('id')->on('tasks')->nullOnDelete();
            $table->index(['application_id', 'created_at']);
        });
    }

    public function down(): void
    {
        Schema::dropIfExists('deployments');
    }
};
