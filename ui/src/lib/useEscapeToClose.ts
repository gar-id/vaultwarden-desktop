import { onMount } from "svelte";
import type { Writable } from "svelte/store";

/**
 * Closes a modal store's boolean flag when Escape is pressed, but only
 * while the modal is actually open — so Escape in one modal never affects
 * another, and the listener is a no-op (and removed) once the component
 * using it unmounts.
 */
export function useEscapeToClose(show: Writable<boolean>): void {
  onMount(() => {
    function handleKeydown(e: KeyboardEvent) {
      if (e.key !== "Escape") return;
      let isOpen = false;
      const unsubscribe = show.subscribe((v) => (isOpen = v));
      unsubscribe();
      if (isOpen) show.set(false);
    }

    window.addEventListener("keydown", handleKeydown);
    return () => window.removeEventListener("keydown", handleKeydown);
  });
}
