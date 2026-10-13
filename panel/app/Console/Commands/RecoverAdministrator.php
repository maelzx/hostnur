<?php

namespace App\Console\Commands;

use App\Enums\AuditAction;
use App\Enums\AuditOutcome;
use App\Models\User;
use App\Services\AuditRecorder;
use Closure;
use Illuminate\Console\Attributes\Description;
use Illuminate\Console\Attributes\Signature;
use Illuminate\Console\Command;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Schema;
use Illuminate\Support\Facades\Validator;
use Illuminate\Support\Str;
use Illuminate\Validation\Rules\Password;
use Illuminate\Validation\ValidationException;
use Throwable;

#[Signature('hostnur:admin:recover')]
#[Description('Reset an existing Hostnur administrator password through a deliberate local recovery')]
class RecoverAdministrator extends Command
{
    public function handle(): int
    {
        try {
            if (! $this->input->isInteractive()) {
                $this->error('Administrator recovery requires an interactive terminal with hidden password input.');

                return self::FAILURE;
            }

            $email = Str::lower(trim((string) $this->ask('Existing administrator email')));

            $administrator = User::query()
                ->where('email', $email)
                ->where('is_admin', true)
                ->first();

            if ($administrator === null) {
                $this->error('No administrator matches that email. Recovery cannot create or promote accounts.');

                return self::FAILURE;
            }

            $this->line("Administrator: {$administrator->name} <{$administrator->email}>");

            if (! $this->confirm('Reset this administrator password? Multi-factor authentication is preserved.', false)) {
                $this->warn('Administrator recovery cancelled. No changes were made.');

                return self::FAILURE;
            }

            $data = Validator::make([
                'password' => $this->secret('New password (at least 12 characters)', false),
                'password_confirmation' => $this->secret('Confirm new password', false),
            ], [
                'password' => [
                    'bail', 'required', 'string', Password::min(12), 'confirmed',
                    function (string $attribute, mixed $value, Closure $fail): void {
                        if (strlen((string) $value) > 72) {
                            $fail('The password must not exceed 72 bytes.');
                        }

                        if (str_contains((string) $value, "\0")) {
                            $fail('The password must not contain null bytes.');
                        }
                    },
                ],
                'password_confirmation' => ['required', 'string'],
            ])->validate();

            $this->recover($administrator, $data['password']);

            $this->info('Administrator password reset. Multi-factor authentication and setup state were preserved, and existing sessions were invalidated.');

            return self::SUCCESS;
        } catch (ValidationException $exception) {
            foreach ($exception->validator->errors()->all() as $error) {
                $this->error($error);
            }

            return self::FAILURE;
        } catch (Throwable) {
            $this->error('Administrator recovery failed. No changes were committed. Check database access and hidden terminal input, then retry.');

            return self::FAILURE;
        }
    }

    private function recover(User $administrator, string $password): void
    {
        DB::transaction(function () use ($administrator, $password): void {
            $fresh = User::query()
                ->whereKey($administrator->getKey())
                ->where('is_admin', true)
                ->lockForUpdate()
                ->first();

            if ($fresh === null) {
                throw ValidationException::withMessages([
                    'recovery' => 'The administrator no longer exists.',
                ]);
            }

            // The hashed cast stores the password; mfa_version is rotated so any
            // existing MFA proof/session is rejected. MFA columns are untouched.
            $fresh->forceFill([
                'password' => $password,
                'mfa_version' => $fresh->mfa_version + 1,
            ])->save();

            if (Schema::hasTable('sessions') && Schema::hasColumn('sessions', 'user_id')) {
                DB::table('sessions')->where('user_id', $fresh->getKey())->delete();
            }

            app(AuditRecorder::class)->record($fresh, AuditAction::AuthAdminRecovered, AuditOutcome::Success);
        });
    }
}
