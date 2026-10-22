<!--
  Application frame. Scaffold only: the start screen and the editor
  workspace are designed with Impeccable (docs/07 §14) before they get any
  visual styling.
-->
<script lang="ts">
  import { i18n } from '$lib/i18n/index.svelte';
</script>

<main>
  <h1>{i18n.t('app.name')}</h1>
  <p>{i18n.t('app.tagline')}</p>
</main>
