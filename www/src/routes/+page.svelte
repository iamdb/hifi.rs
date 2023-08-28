<script>
	import { afterUpdate, onMount } from 'svelte';
	import { fly } from 'svelte/transition';
	import { quintOut } from 'svelte/easing';
	import {
		WS,
		currentTrack,
		currentTrackList,
		isBuffering,
		currentStatus,
		position,
		duration,
		connected
	} from '$lib/websocket';
	import Button from '../lib/components/Button.svelte';
	import { writable } from 'svelte/store';
	import { dev } from '$app/environment';

	$: positionMinutes = Math.floor($position / 1000 / 1000 / 1000 / 60);
	$: positionSeconds = Math.floor($position / 1000 / 1000 / 1000) - positionMinutes * 60;

	$: durationMinutes = Math.floor($duration / 1000 / 1000 / 1000 / 60);
	$: durationSeconds = Math.floor($duration / 1000 / 1000 / 1000) - durationMinutes * 60;

	let controls;

	const showList = writable(false);

	onMount(() => {
		controls = new WS(dev);

		const onFocus = () => {
			if (!controls.connected()) {
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

	let navHeight;
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
		<div
			class="w-full md:w-1/2 md:aspect-square relative bg-amber-900 p-4 lg:p-8 flex-shrink-0 mx-auto flex items-center justify-center"
		>
			<div
				class="w-full h-full p-4 md:p-8 mix-blend-soft-light opacity-75 absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2"
			>
				<img
					class="block w-full h-full max-w-full object-cover"
					src={$currentTrack?.album.image.large}
					alt={$currentTrack?.album.title}
				/>
			</div>
			<div
				class="w-full h-full backdrop-hue-rotate-30 backdrop-contrast-75 backdrop-blur-sm flex flex-col items-center justify-center"
			>
				<img
					class="w-full md:w-auto block max-w-full relative z-10"
					src={$currentTrack?.album.image.large}
					alt={$currentTrack?.album.title}
				/>
			</div>
		</div>
		<div class="flex md:w-1/2 flex-grow flex-col justify-between">
			<div
				class="flex relative flex-col gap-y-4 py-2 flex-grow flex-shrink justify-evenly text-center text-4xl xl:text-6xl"
			>
				{#if $currentTrack}
					<span>{$currentTrack?.track.performer.name || ''}</span>

					<span class="font-semibold py-4 md:py-8 bg-amber-900 leading-[1.15em] px-4 md:px-8"
						>{$currentTrack?.track.title || ''}</span
					>
					<span class="text-4xl md:text-5xl">
						<span>
							{positionMinutes.toString(10).padStart(2, '0')}:{positionSeconds
								.toString(10)
								.padStart(2, '0')}
						</span>
						<span>&nbsp;|&nbsp;</span>
						<span>
							{durationMinutes.toString(10).padStart(2, '0')}:{durationSeconds
								.toString(10)
								.padStart(2, '0')}
						</span>
					</span>
				{/if}

				{#if $showList}
					<div
						on:click|stopPropagation={() => {}}
						on:keyup|stopPropagation={() => {}}
						role="menu"
						tabindex="0"
						style:padding-bottom={`${navHeight + 32}px`}
						transition:fly={{ duration: 300, easing: quintOut, x: '100%' }}
						class="fixed h-[100dvh] md:!pb-0 md:h-full md:absolute z-10 flex flex-col backdrop-blur-sm bg-opacity-90 w-full text-left top-0 right-0 bg-amber-950"
					>
						<div
							class="flex flex-row gap-x-8 py-1 px-2 justify-between text-center text-xl xl:text-4xl"
						>
							<p>{$currentTrack?.track.performer.name}</p>
							<p class="font-bold">{$currentTrack?.album.title}</p>
							<p>{new Date($currentTrack.album.release_date_original).getFullYear()}</p>
						</div>
						<ul class="text-2xl xl:text-3xl px-2 leading-tight overflow-y-scroll">
							{#each $currentTrackList as track}
								<li
									class:opacity-60={track.status === 'Played'}
									class:text-amber-500={track.status === 'Playing'}
								>
									<button
										on:click|stopPropagation={() => controls.skipTo(track.index)}
										class="grid grid-flow-col-dense gap-x-4"
									>
										<span>{(track.index + 1).toString().padStart(2, '0')}</span>
										<span>{track.track.title}</span>
									</button>
								</li>
							{/each}
						</ul>
					</div>
				{/if}
			</div>

			<div
				bind:offsetHeight={navHeight}
				class="flex flex-row gap-x-4 md:mt-0 px-4 md:px-12 items-end justify-between"
			>
				<span class:fixed-button={$showList}>
					<Button onClick={toggleList}>{$showList ? 'Close' : 'List'}</Button>
				</span>
				<div class="flex flex-row justify-end gap-x-4 flex-grow">
					<Button onClick={() => controls?.previous()}>Previous</Button>
					<Button onClick={() => controls?.playPause()}>
						{#if $currentStatus === 'Playing'}
							Pause
						{:else}
							Play
						{/if}
					</Button>
					<Button onClick={() => controls?.next()}>Next</Button>
				</div>
			</div>
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

<style lang="postcss">
	.fixed-button {
		@apply fixed z-20 bottom-4 left-4 md:z-auto md:relative md:bottom-auto md:left-auto;
	}
</style>
