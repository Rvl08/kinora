<!--
  Language and appearance, reachable from the start screen.
-->
<script lang="ts">
  import { LOCALES, type Locale, i18n, storeLocale } from '$lib/i18n/index.svelte';
  import { type ThemePreference, storePreference, theme } from '$lib/theme/theme.svelte';

  const LANGUAGE_NAMES: Readonly<Record<Locale, string>> = {
    id: 'Bahasa Indonesia',
    en: 'English',
  };
  const THEMES: readonly ThemePreference[] = ['system', 'dark', 'light'];
  const THEME_KEYS = {
    system: 'settings.themeSystem',
    dark: 'settings.themeDark',
    light: 'settings.themeLight',
  } as const;

  function setLocale(event: Event) {
    const locale = (event.currentTarget as HTMLSelectElement).value as Locale;
    i18n.locale = locale;
    storeLocale(locale);
  }

  function setTheme(event: Event) {
    const preference = (event.currentTarget as HTMLSelectElement).value as ThemePreference;
    theme.preference = preference;
    storePreference(preference);
  }
</script>

<div class="settings">
  <label>
    <span>{i18n.t('settings.language')}</span>
    <select value={i18n.locale} onchange={setLocale}>
      {#each LOCALES as locale (locale)}
        <option value={locale}>{LANGUAGE_NAMES[locale]}</option>
      {/each}
    </select>
  </label>
  <label>
    <span>{i18n.t('settings.appearance')}</span>
    <select value={theme.preference} onchange={setTheme}>
      {#each THEMES as preference (preference)}
        <option value={preference}>{i18n.t(THEME_KEYS[preference])}</option>
      {/each}
    </select>
  </label>
</div>

<style>
  .settings {
    display: flex;
    gap: var(--space-4);
  }

  label {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    color: var(--text-tertiary);
    font-size: var(--text-xs);
  }

  select {
    min-height: 2rem;
    padding: 0 var(--space-2);
    border: var(--hairline) solid var(--border-subtle);
    border-radius: var(--radius-control);
    background: var(--surface-base);
    color: var(--text-primary);
    font-size: var(--text-xs);
  }

  select:hover {
    border-color: var(--border-control);
  }
</style>
