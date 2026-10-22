// Entry point: applies the language and theme, then mounts the app.

import { mount } from 'svelte';

import './app.css';
import App from './App.svelte';
import { i18n } from './lib/i18n/index.svelte';
import { theme } from './lib/theme/theme.svelte';

document.documentElement.lang = i18n.locale;
theme.start();

const target = document.getElementById('app');
if (!target) throw new Error('#app is missing from index.html');

export default mount(App, { target });
