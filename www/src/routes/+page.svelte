<script>
	import { onMount } from 'svelte';
	import { slide } from 'svelte/transition';
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

		return controls.close;
	});

	const toggleList = () => {
		if ($showList) {
			showList.set(false);
		} else {
			showList.set(true);
		}
	};
</script>

<div class="flex flex-col justify-center h-screen overflow-hidden">
	<div class="flex flex-col h-screen pb-4 sm:py-4 lg:py-0 lg:h-auto justify-between lg:flex-row">
		<div
			class="aspect-square relative lg:w-1/2 bg-amber-800 p-8 flex-shrink-0 mx-auto flex items-center justify-center"
		>
			<div
				class="aspect-square overflow-hidden w-11/12 h-11/12 mix-blend-soft-light opacity-75 absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2"
			>
				<img
					class="block w-full h-full object-cover"
					src={$currentTrack?.album.image.large}
					alt={$currentTrack?.album.title}
				/>
			</div>
			<div
				class="aspect-square w-full h-full backdrop-hue-rotate-30 backdrop-contrast-75 backdrop-blur-sm flex flex-col items-center justify-center"
			>
				<img
					class="block max-w-full relative z-10"
					src={$currentTrack?.album.image.large}
					alt={$currentTrack?.album.title}
				/>
			</div>
		</div>
		<div class="flex lg:w-1/2 flex-grow flex-col justify-between">
			<div
				class="flex flex-col gap-y-4 flex-grow flex-shrink justify-evenly text-center text-4xl xl:text-6xl"
			>
				{#if currentTrack}
					<span>{$currentTrack?.track.performer.name || ''}</span>

					<span class="font-semibold py-8 bg-yellow-800 leading-[1.15em] px-4 lg:px-8"
						>{$currentTrack?.track.title || ''}</span
					>
					<span class="text-4xl lg:text-5xl">
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
			</div>

			<div class="flex flex-row gap-x-4 mt-8 lg:mt-0 px-4 lg:px-12 items-end justify-between">
				<Button onClick={toggleList}>List</Button>
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

{#if $showList}
	<div
		transition:slide={{ duration: 300, easing: quintOut, axis: 'x' }}
		class="fixed top-0 right-0 bg-amber-950 h-screen"
	>
		<ul class="text-2xl py-8 px-12 leading-tight">
			{#each $currentTrackList as track}
				<li
					class="whitespace-nowrap"
					class:opacity-60={track.status === 'Played'}
					class:text-amber-500={track.status === 'Playing'}
				>
					{track.index + 1}
					{track.track.title}
				</li>
			{/each}
		</ul>
	</div>
{/if}

{#if $isBuffering}
	<div class="fixed top-8 right-8 z-10 bg-amber-800 flex px-2 items-center justify-center">
		<h1 class="font-semi text-4xl">BUFFERING</h1>
	</div>
{/if}
{#if !$connected}
	<div class="fixed top-8 right-8 z-10 bg-amber-800 flex px-2 items-center justify-center">
		<h1 class="font-semi text-4xl">DISCONNECTED</h1>
	</div>
{/if}
