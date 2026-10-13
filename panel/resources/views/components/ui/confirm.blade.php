@props([
    'title' => 'Confirm action',
    'confirmLabel' => 'Confirm',
    'method',
    'variant' => 'danger',
])

<div x-data="{ open: false, titleId: $id('confirm-title') }" @keydown.escape.window="open = false">
    <x-ui.button :variant="$variant" type="button" @click="open = true; $nextTick(() => $refs.confirm.focus())">
        {{ $trigger }}
    </x-ui.button>

    <div
        x-show="open"
        x-cloak
        class="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/50 p-4"
        role="dialog"
        aria-modal="true"
        :aria-labelledby="titleId"
    >
        <div class="w-full max-w-md rounded-2xl bg-white p-6 shadow-xl">
            <h2 :id="titleId" class="text-lg font-semibold">{{ $title }}</h2>
            <div class="mt-2 text-sm leading-6 text-slate-600">{{ $slot }}</div>
            <div class="mt-6 flex justify-end gap-3">
                <x-ui.button variant="secondary" type="button" @click="open = false">Cancel</x-ui.button>
                <x-ui.button
                    :variant="$variant"
                    type="button"
                    x-ref="confirm"
                    @click="open = false; $wire.{{ $method }}()"
                >
                    {{ $confirmLabel }}
                </x-ui.button>
            </div>
        </div>
    </div>
</div>
