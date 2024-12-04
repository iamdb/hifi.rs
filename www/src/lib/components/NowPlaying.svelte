<script>
	import {
		currentTrack,
		numOfTracks,
		entityTitle,
		positionString,
		durationString,
		position,
		coverImage
	} from '$lib/websocket';
	import Info from './Info.svelte';
	import { Backward, Forward, Icon, Pause, Play } from 'svelte-hero-icons';
	import { currentStatus } from '$lib/websocket';
	import Marquee from './Marquee.svelte';

	$: progress = ($position / $currentTrack.durationSeconds) * 100;

	export let controls;
</script>

<div class="flex h-full flex-col items-center justify-center gap-8 p-8 landscape:flex-row">
	<div class="aspect-square max-h-full max-w-[600px] overflow-clip rounded-lg shadow-lg">
		<img src={$coverImage} alt={$entityTitle} class="object-contain" />
	</div>

	<div class="flex w-full max-w-md flex-grow flex-col justify-center md:max-w-[600px]">
		<div class="flex items-center gap-2 justify-between">
			<span class="truncate text-xl">
				<Marquee input={$entityTitle} />
			</span>
			<div class="whitespace-nowrap text-gray-500">
				{$currentTrack.number} of {$numOfTracks}
			</div>
		</div>
		<div class="text-gray-400">
			<Marquee input={$currentTrack?.artist.name} />
		</div>

		<div class="flex w-full flex-col gap-y-4">
			<div class="flex items-center justify-between gap-2">
				<span class="truncate text-2xl">
					<Marquee input={$currentTrack?.title} />
				</span>
				<Info explicit={$currentTrack.explicit} hiresAvailable={$currentTrack.hiresAvailable} />
			</div>

			<div>
				<div class="grid h-2 overflow-clip rounded-full">
					<div style="grid-column: 1; grid-row: 1;" class="w-full bg-gray-800"></div>
					<div
						style="grid-column: 1; grid-row: 1;"
						style:width="{progress}%"
						class="bg-gray-500 transition"
					></div>
				</div>
				<div class="flex justify-between text-sm text-gray-500">
					<span>{$positionString}</span>
					<span>{$durationString}</span>
				</div>
			</div>
		</div>

		<div class="flex h-10 flex-row justify-center gap-2">
			<button on:click={() => controls?.previous()}><Icon src={Backward} solid /></button>
			<button on:click={() => controls?.playPause()}>
				{#if $currentStatus === 'Playing'}
					<Icon src={Pause} solid />
				{:else}
					<Icon src={Play} solid />
				{/if}
			</button>
			<button on:click={() => controls?.next()}><Icon src={Forward} solid /></button>
		</div>
	</div>
</div>
