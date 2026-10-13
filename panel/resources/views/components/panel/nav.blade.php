<nav aria-label="Panel" class="border-b border-slate-200 bg-white lg:w-64 lg:shrink-0 lg:border-b-0 lg:border-r">
    <div class="flex h-full flex-col gap-6 p-4 sm:p-6">
        <div>
            <p class="text-sm font-semibold uppercase tracking-widest text-indigo-700">{{ config('app.name') }}</p>
            <p class="mt-1 text-xs text-slate-500">Administrator panel</p>
        </div>

        <ul class="space-y-6 text-sm">
            <li>
                <p class="px-2 text-xs font-semibold uppercase tracking-wide text-slate-400">Overview</p>
                <ul class="mt-2 space-y-1">
                    <li>
                        <a
                            href="{{ route('panel.dashboard') }}"
                            @if (request()->routeIs('panel.dashboard')) aria-current="page" @endif
                            class="block rounded-lg px-2 py-1.5 font-medium {{ request()->routeIs('panel.dashboard') ? 'bg-indigo-50 text-indigo-700' : 'text-slate-700 hover:bg-slate-100' }}"
                        >
                            Dashboard
                        </a>
                    </li>
                </ul>
            </li>

            <li>
                <p class="px-2 text-xs font-semibold uppercase tracking-wide text-slate-400">Tasks</p>
                <ul class="mt-2 space-y-1">
                    <li>
                        <a
                            href="{{ route('panel.tasks.index') }}"
                            @if (request()->routeIs('panel.tasks.*')) aria-current="page" @endif
                            class="block rounded-lg px-2 py-1.5 font-medium {{ request()->routeIs('panel.tasks.*') ? 'bg-indigo-50 text-indigo-700' : 'text-slate-700 hover:bg-slate-100' }}"
                        >
                            Task history
                        </a>
                    </li>
                    <li>
                        <a
                            href="{{ route('panel.audit.index') }}"
                            @if (request()->routeIs('panel.audit.*')) aria-current="page" @endif
                            class="block rounded-lg px-2 py-1.5 font-medium {{ request()->routeIs('panel.audit.*') ? 'bg-indigo-50 text-indigo-700' : 'text-slate-700 hover:bg-slate-100' }}"
                        >
                            Audit history
                        </a>
                    </li>
                </ul>
            </li>

            <li>
                <p class="px-2 text-xs font-semibold uppercase tracking-wide text-slate-400">Security</p>
                <ul class="mt-2 space-y-1">
                    <li>
                        <a
                            href="{{ route('panel.security') }}"
                            @if (request()->routeIs('panel.security')) aria-current="page" @endif
                            class="block rounded-lg px-2 py-1.5 font-medium {{ request()->routeIs('panel.security') ? 'bg-indigo-50 text-indigo-700' : 'text-slate-700 hover:bg-slate-100' }}"
                        >
                            Security settings
                        </a>
                    </li>
                    <li>
                        <a
                            href="{{ route('panel.security.mfa') }}"
                            @if (request()->routeIs('panel.security.mfa')) aria-current="page" @endif
                            class="block rounded-lg px-2 py-1.5 font-medium {{ request()->routeIs('panel.security.mfa') ? 'bg-indigo-50 text-indigo-700' : 'text-slate-700 hover:bg-slate-100' }}"
                        >
                            Multi-factor authentication
                        </a>
                    </li>
                </ul>
            </li>

            <li>
                <p class="px-2 text-xs font-semibold uppercase tracking-wide text-slate-400">Applications</p>
                <p class="mt-2 rounded-lg px-2 py-1.5 text-slate-400">Available in a later phase</p>
            </li>

            <li>
                <p class="px-2 text-xs font-semibold uppercase tracking-wide text-slate-400">Server</p>
                <p class="mt-2 rounded-lg px-2 py-1.5 text-slate-400">Available in a later phase</p>
            </li>
        </ul>

        <form method="POST" action="{{ route('logout') }}" class="mt-auto">
            @csrf
            <button
                type="submit"
                class="w-full rounded-lg border border-slate-300 px-3 py-2 text-sm font-semibold text-slate-700 hover:bg-slate-100 focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-2"
            >
                Sign out
            </button>
        </form>
    </div>
</nav>
