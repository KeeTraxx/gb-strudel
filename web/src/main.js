import { mount } from 'svelte';
import init from './pkg/gb_strudel_wasm.js';
import App from './App.svelte';

await init();
mount(App, { target: document.getElementById('app') });
