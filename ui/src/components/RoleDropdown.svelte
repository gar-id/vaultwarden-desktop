<script lang="ts">
  interface Props {
    value: number; // 1 (Read), 7 (Edit), 15 (Owner)
    onChange: (newValue: number) => void;
  }

  let { value, onChange }: Props = $props();

  let open = $state(false);

  const roles = [
    { value: 1, label: "Can Read" },
    { value: 7, label: "Can Edit" },
    { value: 15, label: "Owner" },
    { value: -1, label: "Remove Access" },
  ];

  let currentRole = $derived(roles.find((r) => r.value === value) ?? roles[0]);

  function selectRole(newVal: number) {
    open = false;
    onChange(newVal);
  }
</script>

<div class="relative shrink-0 select-none">
  <button
    type="button"
    onclick={() => (open = !open)}
    class="bg-gray-800 hover:bg-gray-700 text-gray-100 border border-gray-700 rounded px-2.5 py-1 text-xs flex items-center justify-between gap-2 min-w-[110px] transition-colors"
  >
    <span class={value === -1 ? "text-brand-400 font-medium" : "text-gray-100 font-medium"}>
      {currentRole.label}
    </span>
    <svg xmlns="http://www.w3.org/2000/svg" class="w-3 h-3 text-gray-400 transition-transform {open ? 'rotate-180' : ''}" viewBox="0 0 20 20" fill="currentColor">
      <path fill-rule="evenodd" d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z" clip-rule="evenodd" />
    </svg>
  </button>

  {#if open}
    <!-- Backdrop to close dropdown on outside click -->
    <div
      class="fixed inset-0 z-30"
      onclick={() => (open = false)}
      oncontextmenu={(e) => { e.preventDefault(); open = false; }}
      role="presentation"
    ></div>

    <div class="absolute right-0 top-full mt-1 w-36 bg-gray-900 border border-gray-800 rounded-lg shadow-2xl py-1 z-40 text-xs">
      {#each roles as r}
        <button
          type="button"
          onclick={() => selectRole(r.value)}
          class="w-full text-left px-3 py-1.5 transition-colors flex items-center justify-between
                 {r.value === -1 ? 'text-brand-400 hover:bg-gray-800 hover:text-brand-300 font-medium' : 'text-gray-200 hover:bg-gray-800 hover:text-white'}
                 {r.value === value ? 'bg-gray-800/80 font-semibold text-white' : ''}"
        >
          <span>{r.label}</span>
          {#if r.value === value}
            <svg xmlns="http://www.w3.org/2000/svg" class="w-3.5 h-3.5 text-brand-500" viewBox="0 0 20 20" fill="currentColor">
              <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" />
            </svg>
          {/if}
        </button>
      {/each}
    </div>
  {/if}
</div>
