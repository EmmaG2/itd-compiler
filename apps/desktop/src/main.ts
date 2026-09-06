import { mount } from 'svelte'
import App from './App.svelte'
import './styles.css'
const target = document.querySelector<HTMLElement>('#app')
if (!target) throw new Error('Missing application root')
mount(App, { target })
