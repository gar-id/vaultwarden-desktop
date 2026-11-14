<script lang="ts">
  import { showPasswordGen } from "../lib/stores";
  import { generatePassword } from "../lib/tauri";
  import type { PasswordRules } from "../lib/types";
  import { useEscapeToClose } from "../lib/useEscapeToClose";

  useEscapeToClose(showPasswordGen);

  let rules = $state<PasswordRules>({
    length: 20,
    uppercase: true,
    lowercase: true,
    numbers: true,
    symbols: false,
  });
  let output = $state("");
  let copied = $state(false);

  async function regen() {
    try {
      output = await generatePassword(rules);
    } catch (_e) {
      output = "";
    }
  }

  $effect(() => {
    // re-run whenever any rule changes
    const _r = rules.length + +rules.uppercase + +rules.lowercase + +rules.numbers + +rules.symbols;
    regen();
  });

  async function handleCopy() {
    await navigator.clipboard.writeText(output);
    copied = true;
    setTimeout(() => (copied = false), 2000);
  }
</script>

{#if $showPasswordGen}
  <div class="fixed inset-x-0 bottom-0 top-9 bg-black/70 flex items-center justify-center z-50">
    <div class="bg-gray-900 rounded-lg p-6 w-96 space-y-4 shadow-2xl">
      <h2 class="text-lg font-semibold text-gray-100">Password Generator</h2>

      <div class="bg-gray-800 rounded p-3 font-mono text-sm text-gray-100 break-all min-h-10">
        {output || "—"}
      </div>

      <div>
        <label class="text-xs text-gray-400 uppercase tracking-wide" for="pg-length">Length: {rules.length}</label>
        <input id="pg-length" type="range" min="8" max="128" bind:value={rules.length}
          class="w-full mt-1" />
      </div>

      <div class="grid grid-cols-2 gap-2">
        <label class="flex items-center gap-2 text-sm text-gray-300 cursor-pointer">
          <input type="checkbox" bind:checked={rules.uppercase} class="accent-brand-500" />
          Uppercase A–Z
        </label>
        <label class="flex items-center gap-2 text-sm text-gray-300 cursor-pointer">
          <input type="checkbox" bind:checked={rules.lowercase} class="accent-brand-500" />
          Lowercase a–z
        </label>
        <label class="flex items-center gap-2 text-sm text-gray-300 cursor-pointer">
          <input type="checkbox" bind:checked={rules.numbers} class="accent-brand-500" />
          Numbers 0–9
        </label>
        <label class="flex items-center gap-2 text-sm text-gray-300 cursor-pointer">
          <input type="checkbox" bind:checked={rules.symbols} class="accent-brand-500" />
          Symbols !@#…
        </label>
      </div>

      <div class="flex justify-end gap-2">
        <button onclick={() => showPasswordGen.set(false)}
          class="px-3 py-1.5 text-sm text-gray-400 hover:text-gray-200 transition-colors">Close</button>
        <button onclick={regen}
          class="px-3 py-1.5 text-sm text-gray-600 hover:text-gray-300 transition-colors">Regenerate</button>
        <button onclick={handleCopy}
          class="px-4 py-1.5 text-sm bg-brand-600 hover:bg-brand-500 rounded font-medium transition-colors">
          {copied ? "Copied!" : "Copy"}
        </button>
      </div>
    </div>
  </div>
{/if}
