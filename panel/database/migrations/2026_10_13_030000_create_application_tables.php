<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::create('source_credentials', function (Blueprint $table) {
            $table->uuid('id')->primary();
            $table->string('label');
            $table->string('type');
            $table->text('secret');
            $table->text('public_key')->nullable();
            $table->text('host_key')->nullable();
            $table->unsignedInteger('version')->default(1);
            $table->timestamp('rotated_at')->nullable();
            $table->timestamps();
        });

        Schema::create('applications', function (Blueprint $table) {
            $table->uuid('id')->primary();
            $table->string('name');
            $table->string('slug')->unique();
            $table->string('provider');
            $table->string('runtime');
            $table->string('runtime_version')->nullable();
            $table->string('repository_url')->nullable();
            $table->string('git_ref')->nullable();
            $table->uuid('source_credential_id')->nullable();
            $table->string('source_status')->default('not_checked');
            $table->unsignedInteger('current_revision')->default(1);
            $table->string('observed_state')->nullable();
            $table->string('observed_mode')->nullable();
            $table->timestamp('observed_at')->nullable();
            $table->timestamp('archived_at')->nullable();
            $table->timestamps();

            $table->foreign('source_credential_id')->references('id')->on('source_credentials')->nullOnDelete();
            $table->index('archived_at');
        });

        Schema::create('application_revisions', function (Blueprint $table) {
            $table->uuid('id')->primary();
            $table->foreignUuid('application_id')->constrained('applications')->cascadeOnDelete();
            $table->unsignedInteger('revision');
            $table->json('settings');
            $table->json('credential_versions')->nullable();
            $table->foreignId('created_by')->nullable()->constrained('users')->nullOnDelete();
            $table->timestamp('created_at')->nullable();

            $table->unique(['application_id', 'revision']);
        });

        Schema::create('application_secrets', function (Blueprint $table) {
            $table->uuid('id')->primary();
            $table->foreignUuid('application_id')->constrained('applications')->cascadeOnDelete();
            $table->string('name');
            $table->text('value');
            $table->unsignedInteger('version')->default(1);
            $table->timestamp('rotated_at')->nullable();
            $table->timestamps();

            $table->unique(['application_id', 'name']);
        });

        Schema::create('application_domains', function (Blueprint $table) {
            $table->uuid('id')->primary();
            $table->foreignUuid('application_id')->constrained('applications')->cascadeOnDelete();
            $table->string('hostname')->unique();
            $table->string('https_policy')->default('auto');
            $table->string('observed_routing_state')->nullable();
            $table->string('observed_certificate_state')->nullable();
            $table->string('observed_provenance')->nullable();
            $table->timestamps();
        });
    }

    public function down(): void
    {
        Schema::dropIfExists('application_domains');
        Schema::dropIfExists('application_secrets');
        Schema::dropIfExists('application_revisions');
        Schema::dropIfExists('applications');
        Schema::dropIfExists('source_credentials');
    }
};
