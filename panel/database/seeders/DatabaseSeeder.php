<?php

namespace Database\Seeders;

use Illuminate\Database\Console\Seeds\WithoutModelEvents;
use Illuminate\Database\Seeder;

class DatabaseSeeder extends Seeder
{
    use WithoutModelEvents;

    /**
     * Seed application data without creating default administrator accounts.
     *
     * The DemoSeeder is opt-in (run explicitly with `--class=Database\Seeders\DemoSeeder`)
     * and creates a local-only administrator plus sample data for UI work; it is
     * never run by the default `--seed` to avoid touching a real database.
     */
    public function run(): void {}
}
