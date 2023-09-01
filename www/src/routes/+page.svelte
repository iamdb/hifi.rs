<script>
	import { onMount } from 'svelte';
	import {
		WS,
		currentTrack,
		isBuffering,
		currentStatus,
		connected,
		coverImage,
		entityTitle
	} from '$lib/websocket';
	import { writable } from 'svelte/store';
	import { dev } from '$app/environment';
	import CoverArt from '../lib/components/CoverArt.svelte';
	import Navigation from '../lib/components/Navigation.svelte';
	import TrackMetadata from '../lib/components/TrackMetadata.svelte';
	import TrackList from '../lib/components/TrackList.svelte';

	let controls;

	const showList = writable(false);

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

	const toggleList = () => {
		if ($showList) {
			showList.set(false);
		} else {
			showList.set(true);
		}
	};

	const navHeight = writable(0);
</script>

<svelte:head>
	<title>hifi.rs: {$currentStatus}</title>
</svelte:head>

<svelte:body
	on:click={(e) => {
		if (e.currentTarget !== document.getElementsByTagName('body') && $showList) {
			toggleList();
		}
	}}
/>

<div class="flex flex-col justify-center h-[100dvh] overflow-x-hidden">
	<div class="flex flex-col h-[100dvh] md:h-auto pb-4 sm:py-4 md:py-0 justify-between md:flex-row">
		<CoverArt src={$coverImage} alt={$entityTitle} />
		<div class="flex md:w-1/2 flex-grow flex-col justify-between">
			<div
				class="flex relative flex-col gap-y-4 py-2 flex-grow flex-shrink justify-evenly text-center text-4xl xl:text-6xl"
			>
				{#if $currentTrack}
					<TrackMetadata />
				{/if}

				<TrackList {showList} {navHeight} {controls} />
			</div>

			<Navigation {showList} {toggleList} {controls} {navHeight} />
		</div>
	</div>
</div>

{#if $isBuffering || !$connected}
	<div class="fixed top-8 right-8 z-10">
		{#if $isBuffering}
			<h1 class="font-semi text-4xl bg-amber-800 leading-none p-2">BUFFERING</h1>
		{/if}
		{#if !$connected}
			<h1 class="font-semi text-4xl bg-amber-800 leading-none p-2">DISCONNECTED</h1>
		{/if}
	</div>
{/if}
