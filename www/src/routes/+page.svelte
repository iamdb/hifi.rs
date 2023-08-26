<script>
	import { onMount } from 'svelte';
	import { slide } from 'svelte/transition';
	import { quintOut } from 'svelte/easing';
	import {
		init,
		currentTrack,
		currentTrackList,
		isBuffering,
		currentStatus,
		position
	} from '$lib/websocket';
	import Button from '../lib/components/Button.svelte';
	import { writable } from 'svelte/store';
	import { dev } from '$app/environment';

	$: clockMinutes = Math.floor($position / 1000 / 1000 / 1000 / 60);
	$: clockSeconds = Math.floor($position / 1000 / 1000 / 1000) - clockMinutes * 60;

	let controls;

	const showList = writable(false);

	onMount(() => {
		controls = init(dev);

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

<div>
	<div class="flex flex-col lg:flex-row">
		<div
			class="aspect-square lg:w-1/2 bg-amber-800 p-8 flex-shrink-0 flex-grow mx-auto flex items-center justify-center"
		>
			<img
				class="w-full block h-full object-cover max-w-full"
				src={$currentTrack?.album.image.large}
				alt={$currentTrack?.album.title}
			/>
		</div>
		<div class="flex lg:w-1/2 flex-col justify-between">
			<div class="flex flex-col flex-grow justify-center text-center text-7xl lg:text-7xl">
				{#if currentTrack}
					<span
						class="font-bold py-8 tracking-tighter bg-yellow-800 leading-tight px-8 lg:text-8xl italic"
						>{$currentTrack?.track.title || ''}</span
					>
					<span class="font-bold italic">{$currentTrack?.track.performer.name || ''}</span>

					<span class="font-mono">
						{clockMinutes.toString(10).padStart(2, '0')}:{clockSeconds
							.toString(10)
							.padStart(2, '0')}
					</span>
				{/if}
			</div>

			<div class="flex flex-row px-12 items-end justify-between">
				<Button onClick={toggleList}>List</Button>
				<div class="flex flex-row justify-end gap-x-4 flex-grow">
					<Button onClick={controls?.previous}>Previous</Button>
					<Button onClick={controls?.playPause}>
						{#if $currentStatus === 'Playing'}
							Pause
						{:else}
							Play
						{/if}
					</Button>
					<Button onClick={controls?.next}>Next</Button>
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
	<div
		class="fixed top-1/2 left-1/2 -translate-y-1/2 -translate-x-1/2 z-10 w-3/5 h-3/5 bg-amber-800 flex items-center justify-center"
	>
		<h1 class="font-bold text-8xl">BUFFERING</h1>
	</div>
{/if}
