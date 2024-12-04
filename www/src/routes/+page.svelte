<script>
	import { onMount } from 'svelte';
	import {
		WS,
		currentTrack,
		isBuffering,
		isLoading,
		currentStatus,
		connected
	} from '$lib/websocket';
	import { writable } from 'svelte/store';
	import { dev } from '$app/environment';
	import Navigation from '../lib/components/Navigation.svelte';
	import Queue from '../lib/components/Queue.svelte';
	import { Icon, LinkSlash, ArrowPath } from 'svelte-hero-icons';
	import NowPlaying from '../lib/components/NowPlaying.svelte';
	import Search from '../lib/components/Search.svelte';
	import Favorites from '../lib/components/Favorites.svelte';

	let controls;

	const activePage = writable('nowPlaying');
	const setPage = (newPage) => activePage.set(newPage);

	onMount(() => {
		controls = new WS(dev);

		const onFocus = () => {
			if (!$connected) {
				controls.connect();
			}
		};

		window.addEventListener('focus', onFocus);

		return () => {
			controls.close();
			window.removeEventListener('focus', onFocus);
		};
	});
</script>

<svelte:head>
	<title>hifi.rs: {$currentStatus}</title>
</svelte:head>

<div class="flex flex-col justify-between h-full pt-safe px-safe">
	<div class="flex overflow-hidden flex-col justify-between h-full">
		{#if $activePage == 'nowPlaying' && $currentTrack}
			<NowPlaying {controls} />
		{/if}

		{#if $activePage == 'search'}
			<Search {controls} />
		{/if}

		{#if $activePage == 'favorites'}
			<Favorites {controls} />
		{/if}

		{#if $activePage == 'queue'}
			<Queue {controls} />
		{/if}
	</div>
	<Navigation {setPage} {activePage} />
</div>

{#if $isBuffering || !$connected || $isLoading}
	<div class="fixed top-8 right-8 z-10 p-2 bg-black bg-opacity-20 rounded backdrop-blur size-12">
		{#if !$connected}
			<Icon src={LinkSlash} solid />
		{:else if $isLoading || $isBuffering}
			<Icon src={ArrowPath} solid class="animate-spin" />
		{/if}
	</div>
{/if}
