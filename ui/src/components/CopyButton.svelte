<script lang="ts">
  interface Props {
    // Either a plain plaintext value to copy directly (username, URL,
    // notes, TOTP code - already decrypted/known client-side, no need to
    // round-trip through Rust), or an async action for cases that must
    // go through the backend (the password, which uses copy_secret's
    // 30s auto-clear rather than a plain clipboard write).
    value?: string;
    onCopy?: () => Promise<void>;
    title?: string;
  }

  let { value, onCopy, title = "Copy" }: Props = $props();

  let copied = $state(false);
  let resetTimer: ReturnType<typeof setTimeout>;

  async function handleClick() {
    if (onCopy) {
      await onCopy();
    } else if (value) {
      await navigator.clipboard.writeText(value);
    } else {
      return;
    }
    copied = true;
    clearTimeout(resetTimer);
    resetTimer = setTimeout(() => (copied = false), 1500);
  }
</script>

<button
  onclick={handleClick}
  {title}
  class="shrink-0 w-7 h-7 flex items-center justify-center rounded transition-colors
         {copied ? 'text-brand-400' : 'text-gray-500 hover:text-gray-200 hover:bg-gray-800'}"
>
  {#if copied}
    <svg xmlns="http://www.w3.org/2000/svg" class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <path d="M20 6L9 17l-5-5" />
    </svg>
  {:else}
    <svg xmlns="http://www.w3.org/2000/svg" class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <rect x="9" y="9" width="12" height="12" rx="2" />
      <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
    </svg>
  {/if}
</button>
