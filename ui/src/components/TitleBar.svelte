<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { theme, toggleTheme } from "../lib/stores";

  const win = getCurrentWindow();
  let maximized = $state(false);

  win.isMaximized().then((v) => (maximized = v));
  win.onResized(() => {
    win.isMaximized().then((v) => (maximized = v));
  });

  function handleMouseDown(e: MouseEvent) {
    // Ignore drags that started on a button (minimize/maximize/close) -
    // otherwise clicking them also starts a window drag.
    if ((e.target as HTMLElement).closest("button")) return;
    if (e.button !== 0) return;
    win.startDragging();
  }

  function handleDoubleClick() {
    win.toggleMaximize();
  }
</script>

<!-- Custom in-webview title bar, replacing native OS chrome
     (decorations: false in tauri.conf.json) - the native GTK/Windows/
     macOS title bar didn't match the app's red-on-black theme. This
     renders consistently across Linux/Windows/macOS instead of relying
     on per-OS overlay hints that Linux doesn't support anyway. -->
<div
  class="flex items-center h-9 bg-gray-950 border-b border-gray-800 shrink-0 select-none relative z-[60]"
  onmousedown={handleMouseDown}
  ondblclick={handleDoubleClick}
  role="presentation"
>
  <div class="flex items-center gap-2 px-3 pointer-events-none">
    <img src="/passbolt.svg" alt="" class="w-4 h-4" />
    <span class="text-xs font-medium text-gray-400">Passbolt Desktop</span>
  </div>

  <div class="flex-1"></div>

  <div class="flex items-center h-full">
    <button
      type="button"
      onclick={toggleTheme}
      title={$theme === "dark" ? "Switch to light mode" : "Switch to dark mode"}
      class="w-9 h-full flex items-center justify-center text-gray-500 hover:text-gray-200 hover:bg-gray-800 transition-colors"
    >
      {#if $theme === "dark"}
        <svg xmlns="http://www.w3.org/2000/svg" class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="12" cy="12" r="5" />
          <line x1="12" y1="1" x2="12" y2="3" />
          <line x1="12" y1="21" x2="12" y2="23" />
          <line x1="4.22" y1="4.22" x2="5.64" y2="5.64" />
          <line x1="18.36" y1="18.36" x2="19.78" y2="19.78" />
          <line x1="1" y1="12" x2="3" y2="12" />
          <line x1="21" y1="12" x2="23" y2="12" />
          <line x1="4.22" y1="19.78" x2="5.64" y2="18.36" />
          <line x1="18.36" y1="5.64" x2="19.78" y2="4.22" />
        </svg>
      {:else}
        <svg xmlns="http://www.w3.org/2000/svg" class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
        </svg>
      {/if}
    </button>
    <button
      onclick={() => win.minimize()}
      title="Minimize"
      class="w-10 h-full flex items-center justify-center text-gray-500 hover:text-gray-200 hover:bg-gray-800 transition-colors"
    >
      <svg xmlns="http://www.w3.org/2000/svg" class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M5 12h14" />
      </svg>
    </button>
    <button
      onclick={() => win.toggleMaximize()}
      title={maximized ? "Restore" : "Maximize"}
      class="w-10 h-full flex items-center justify-center text-gray-500 hover:text-gray-200 hover:bg-gray-800 transition-colors"
    >
      {#if maximized}
        <svg xmlns="http://www.w3.org/2000/svg" class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <rect x="6" y="6" width="12" height="12" rx="1" />
          <path d="M9 6V5a1 1 0 0 1 1-1h8a1 1 0 0 1 1 1v8a1 1 0 0 1-1 1h-1" />
        </svg>
      {:else}
        <svg xmlns="http://www.w3.org/2000/svg" class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <rect x="4" y="4" width="16" height="16" rx="1" />
        </svg>
      {/if}
    </button>
    <button
      onclick={() => win.close()}
      title="Close"
      class="w-10 h-full flex items-center justify-center text-gray-500 hover:text-white hover:bg-brand-600 transition-colors"
    >
      <svg xmlns="http://www.w3.org/2000/svg" class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M6 6l12 12M18 6L6 18" />
      </svg>
    </button>
  </div>
</div>
