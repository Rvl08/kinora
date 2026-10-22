// Svelte compiler options shared by Vite, Vitest and svelte-check.
/** @type {import('@sveltejs/vite-plugin-svelte').SvelteConfig} */
export default {
  compilerOptions: {
    // Svelte 5 runes everywhere; legacy reactivity is not allowed.
    runes: true,
  },
};
