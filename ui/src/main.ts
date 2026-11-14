import "./app.css";
import App from "./App.svelte";
import { mount } from "svelte";
import { initLogger } from "./lib/logger";

// Initialize before mounting so any error during the very first render
// (e.g. a rejected IPC call in App.svelte's onMount) is still captured.
void initLogger();

const app = mount(App, { target: document.getElementById("app")! });
export default app;
