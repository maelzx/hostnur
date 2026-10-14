@php
    $item = 'flex items-center rounded-lg px-3 py-2 text-sm font-medium transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-400';
    $active = 'bg-indigo-600 text-white';
    $inactive = 'text-slate-300 hover:bg-white/5 hover:text-white';
    $label = 'px-3 text-xs font-semibold uppercase tracking-wider text-slate-500';
    $is = fn (string ...$patterns): bool => request()->routeIs(...$patterns);
@endphp

<nav aria-label="Panel" class="flex shrink-0 flex-col bg-slate-900 lg:sticky lg:top-0 lg:h-dvh lg:w-64 lg:overflow-y-auto">
    <div class="flex items-center gap-2.5 p-4 sm:p-5">
        <span class="inline-flex h-9 w-9 items-center justify-center rounded-lg bg-indigo-600 text-base font-bold text-white" aria-hidden="true">H</span>
        <div>
            <p class="text-sm font-semibold tracking-tight text-white">{{ config('app.name') }}</p>
            <p class="text-xs text-slate-400">Administrator panel</p>
        </div>
    </div>

    <div class="flex-1 space-y-6 px-3 pb-4">
        <div>
            <p class="{{ $label }}">Overview</p>
            <ul class="mt-2 space-y-1">
                <li>
                    <a href="{{ route('panel.dashboard') }}" @if ($is('panel.dashboard')) aria-current="page" @endif
                       class="{{ $item }} {{ $is('panel.dashboard') ? $active : $inactive }}">Dashboard</a>
                </li>
            </ul>
        </div>

        <div>
            <p class="{{ $label }}">Applications</p>
            <ul class="mt-2 space-y-1">
                <li>
                    <a href="{{ route('panel.applications.index') }}" @if ($is('panel.applications.*')) aria-current="page" @endif
                       class="{{ $item }} {{ $is('panel.applications.*') ? $active : $inactive }}">Applications</a>
                </li>
                <li>
                    <a href="{{ route('panel.deployments.index') }}" @if ($is('panel.deployments.*')) aria-current="page" @endif
                       class="{{ $item }} {{ $is('panel.deployments.*') ? $active : $inactive }}">Deployments</a>
                </li>
            </ul>
        </div>

        <div>
            <p class="{{ $label }}">Server</p>
            <ul class="mt-2 space-y-1">
                @foreach ([
                    'panel.server' => 'Server overview',
                    'panel.schedules.index' => 'Scheduled jobs',
                    'panel.certificates.index' => 'Domains & TLS',
                    'panel.databases.index' => 'Databases',
                    'panel.backups.index' => 'Backups',
                ] as $name => $text)
                    <li>
                        <a href="{{ route($name) }}" @if ($is($name)) aria-current="page" @endif
                           class="{{ $item }} {{ $is($name) ? $active : $inactive }}">{{ $text }}</a>
                    </li>
                @endforeach
            </ul>
        </div>

        <div>
            <p class="{{ $label }}">Activity</p>
            <ul class="mt-2 space-y-1">
                <li>
                    <a href="{{ route('panel.tasks.index') }}" @if ($is('panel.tasks.*')) aria-current="page" @endif
                       class="{{ $item }} {{ $is('panel.tasks.*') ? $active : $inactive }}">Task history</a>
                </li>
                <li>
                    <a href="{{ route('panel.audit.index') }}" @if ($is('panel.audit.*')) aria-current="page" @endif
                       class="{{ $item }} {{ $is('panel.audit.*') ? $active : $inactive }}">Audit history</a>
                </li>
            </ul>
        </div>

        <div>
            <p class="{{ $label }}">Security</p>
            <ul class="mt-2 space-y-1">
                <li>
                    <a href="{{ route('panel.security') }}" @if ($is('panel.security')) aria-current="page" @endif
                       class="{{ $item }} {{ $is('panel.security') ? $active : $inactive }}">Security settings</a>
                </li>
                <li>
                    <a href="{{ route('panel.security.mfa') }}" @if ($is('panel.security.mfa')) aria-current="page" @endif
                       class="{{ $item }} {{ $is('panel.security.mfa') ? $active : $inactive }}">Multi-factor authentication</a>
                </li>
            </ul>
        </div>
    </div>

    <form method="POST" action="{{ route('logout') }}" class="border-t border-white/10 p-3">
        @csrf
        <button
            type="submit"
            class="w-full rounded-lg border border-white/15 px-3 py-2 text-sm font-semibold text-slate-300 transition-colors hover:bg-white/5 hover:text-white focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-400"
        >
            Sign out
        </button>
    </form>
</nav>
