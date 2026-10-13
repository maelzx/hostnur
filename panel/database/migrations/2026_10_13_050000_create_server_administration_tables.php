<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::create('scheduled_jobs', function (Blueprint $table) {
            $table->uuid('id')->primary();
            $table->foreignUuid('application_id')->constrained('applications')->cascadeOnDelete();
            $table->string('name');
            $table->string('expression');
            $table->string('timezone');
            $table->string('command_reference');
            $table->boolean('enabled')->default(true);
            $table->unsignedInteger('revision')->default(1);
            $table->timestamps();

            $table->unique(['application_id', 'name']);
        });

        Schema::create('certificates', function (Blueprint $table) {
            $table->uuid('id')->primary();
            $table->uuid('application_domain_id')->nullable();
            $table->string('hostname');
            $table->string('source');
            $table->string('provider')->nullable();
            $table->text('certificate_pem')->nullable();
            $table->text('private_key')->nullable();
            $table->text('passphrase')->nullable();
            $table->timestamp('issued_at')->nullable();
            $table->timestamp('expires_at')->nullable();
            $table->string('state')->default('requested');
            $table->uuid('task_id')->nullable();
            $table->string('observed_state')->nullable();
            $table->string('observed_provenance')->nullable();
            $table->timestamps();

            $table->foreign('application_domain_id')->references('id')->on('application_domains')->nullOnDelete();
            $table->foreign('task_id')->references('id')->on('tasks')->nullOnDelete();
            $table->index('hostname');
        });

        Schema::create('database_requests', function (Blueprint $table) {
            $table->uuid('id')->primary();
            $table->foreignUuid('application_id')->constrained('applications')->cascadeOnDelete();
            $table->string('engine');
            $table->string('database_name');
            $table->string('username');
            $table->text('password');
            $table->string('state')->default('requested');
            $table->uuid('task_id')->nullable();
            $table->timestamps();

            $table->foreign('task_id')->references('id')->on('tasks')->nullOnDelete();
        });

        Schema::create('backup_policies', function (Blueprint $table) {
            $table->uuid('id')->primary();
            $table->foreignUuid('application_id')->nullable()->constrained('applications')->cascadeOnDelete();
            $table->string('scope');
            $table->string('expression');
            $table->string('timezone');
            $table->unsignedInteger('retention_days')->default(7);
            $table->uuid('destination_credential_id')->nullable();
            $table->boolean('enabled')->default(true);
            $table->timestamps();

            $table->foreign('destination_credential_id')->references('id')->on('source_credentials')->nullOnDelete();
        });

        Schema::create('backups', function (Blueprint $table) {
            $table->uuid('id')->primary();
            $table->uuid('backup_policy_id')->nullable();
            $table->uuid('application_id')->nullable();
            $table->string('mode');
            $table->string('state')->default('requested');
            $table->string('verification_status')->default('unverified');
            $table->unsignedBigInteger('size_bytes')->nullable();
            $table->uuid('task_id')->nullable();
            $table->timestamps();

            $table->foreign('backup_policy_id')->references('id')->on('backup_policies')->nullOnDelete();
            $table->foreign('application_id')->references('id')->on('applications')->nullOnDelete();
            $table->foreign('task_id')->references('id')->on('tasks')->nullOnDelete();
        });
    }

    public function down(): void
    {
        Schema::dropIfExists('backups');
        Schema::dropIfExists('backup_policies');
        Schema::dropIfExists('database_requests');
        Schema::dropIfExists('certificates');
        Schema::dropIfExists('scheduled_jobs');
    }
};
