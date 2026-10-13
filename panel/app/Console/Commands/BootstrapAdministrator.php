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
use RuntimeException;
use Throwable;

#[Signature('hostnur:admin:bootstrap')]
#[Description('Create the first Hostnur administrator through a one-time local setup')]
class BootstrapAdministrator extends Command
{
    public function handle(): int
    {
        try {
            if (! Schema::hasTable('panel_setup')) {
                $this->error('Run php artisan migrate before administrator setup.');

                return self::FAILURE;
            }

            $setup = DB::table('panel_setup')->where('id', 1)->first();

            if ($setup === null) {
                $this->error('Panel setup state is missing. Restore it before retrying.');

                return self::FAILURE;
            }

            if ($setup->administrator_bootstrapped_at !== null) {
                $this->error('Administrator setup has already been completed.');

                return self::FAILURE;
            }

            if (User::query()->exists()) {
                $this->error('Accounts already exist. Bootstrap cannot create another account.');

                return self::FAILURE;
            }

            if (! $this->input->isInteractive()) {
                $this->error('Administrator setup requires an interactive terminal with hidden password input.');

                return self::FAILURE;
            }

            $data = Validator::make([
                'name' => trim((string) $this->ask('Administrator name')),
                'email' => Str::lower(trim((string) $this->ask('Administrator email'))),
                'password' => $this->secret('Password (at least 12 characters)', false),
                'password_confirmation' => $this->secret('Confirm password', false),
            ], [
                'name' => ['required', 'string', 'max:255'],
                'email' => ['required', 'string', 'email:rfc', 'max:254'],
                'password' => [
                    'bail', 'required', 'string', Password::min(12), 'confirmed',
                    function (string $attribute, mixed $value, Closure $fail): void {
                        if (strlen($value) > 72) {
                            $fail('The password must not exceed 72 bytes.');
                        }

                        if (str_contains($value, "\0")) {
                            $fail('The password must not contain null bytes.');
                        }
                    },
                ],
                'password_confirmation' => ['required', 'string'],
            ])->validate();

            $this->createAdministrator($data);
            $this->info('Hostnur administrator created. Setup is now closed.');

            return self::SUCCESS;
        } catch (ValidationException $exception) {
            foreach ($exception->validator->errors()->all() as $error) {
                $this->error($error);
            }

            return self::FAILURE;
        } catch (Throwable) {
            $this->error('Administrator setup failed. No changes were committed. Check database access and hidden terminal input, then retry.');

            return self::FAILURE;
        }
    }

    /**
     * @param  array{name: string, email: string, password: string, password_confirmation: string}  $data
     */
    private function createAdministrator(array $data): void
    {
        DB::transaction(function () use ($data): void {
            $claimed = DB::table('panel_setup')
                ->where('id', 1)
                ->whereNull('administrator_bootstrapped_at')
                ->update(['administrator_bootstrapped_at' => now()]);

            if ($claimed !== 1) {
                throw ValidationException::withMessages([
                    'bootstrap' => 'Administrator setup has already been completed or its state is missing.',
                ]);
            }

            if (User::query()->exists()) {
                throw ValidationException::withMessages([
                    'bootstrap' => 'Accounts already exist. Bootstrap cannot create another account.',
                ]);
            }

            $administrator = new User([
                'name' => $data['name'],
                'email' => $data['email'],
                'password' => $data['password'],
            ]);
            $administrator->is_admin = true;

            if (! $administrator->save()) {
                throw new RuntimeException('Administrator creation was cancelled.');
            }

            app(AuditRecorder::class)->record($administrator, AuditAction::AuthAdminBootstrapped, AuditOutcome::Success);
        });
    }
}
