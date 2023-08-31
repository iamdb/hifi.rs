<script>
	import { afterUpdate } from 'svelte';
	import {
		currentTrack,
		numOfTracks,
		entityTitle,
		positionString,
		durationString
	} from '$lib/websocket';
	import { writable } from 'svelte/store';

	let titleWidth, titleWrapperWidth;

	const enableMarquee = writable(false);

	afterUpdate(() => {
		if (titleWidth > titleWrapperWidth) {
			enableMarquee.set(true);
		} else {
			enableMarquee.set(false);
		}
	});
</script>

<div class="flex flex-col items-center">
	<div>{$entityTitle || ''}</div>
	<div class="text-4xl text-amber-600">
		<span class="text-2xl align-baseline">by</span>
		{$currentTrack?.artist.name || ''}
	</div>
	<div class="text-2xl xl:text-4xl mt-4 xl:mt-8 flex flex-row items-center gap-x-8">
		<span class="font-bold bg-blue-950 px-2">{$currentTrack.number}</span>
		<span class="text-2xl">of</span>
		<span class="font-bold bg-blue-950 px-2">{$numOfTracks}</span>
	</div>
</div>

<div
	bind:offsetWidth={titleWrapperWidth}
	class:justify-center={!$enableMarquee}
	class="bg-amber-900 overflow-hidden flex flex-row"
>
	<div
		class:marquee={$enableMarquee}
		class:pl-[50%]={$enableMarquee}
		class="md:py-4 flex flex-row leading-[1.15em] xl:py-8 font-semibold py-2 whitespace-nowrap"
	>
		<span bind:offsetWidth={titleWidth}>
			{$currentTrack?.title || ''}
			{#if $currentTrack.explicit}
				<svg
					class="inline-block"
					xmlns="http://www.w3.org/2000/svg"
					width="24"
					height="24"
					viewBox="0 0 24 24"
					><path fill="currentColor" d="M21 3H3v18h18V3zm-6 6h-4v2h4v2h-4v2h4v2H9V7h6v2z" /></svg
				>
			{/if}
		</span>
	</div>

	{#if $enableMarquee}
		<div
			class:marquee={$enableMarquee}
			class:pl-[50%]={$enableMarquee}
			class="md:py-4 flex flex-row leading-[1.15em] xl:py-8 font-semibold py-2 whitespace-nowrap"
		>
			{$currentTrack?.title || ''}
			{#if $currentTrack.explicit}
				<svg
					class="inline-block align-middle"
					xmlns="http://www.w3.org/2000/svg"
					width="24"
					height="24"
					viewBox="0 0 24 24"
					><path fill="currentColor" d="M21 3H3v18h18V3zm-6 6h-4v2h4v2h-4v2h4v2H9V7h6v2z" /></svg
				>
			{/if}
		</div>
	{/if}
</div>

<div class="flex flex-col gap-y-4 max-w-xs mx-auto">
	<div class="text-4xl md:text-5xl grid grid-cols-3">
		<span>
			{$positionString}
		</span>
		<span>&nbsp;|&nbsp;</span>
		<span>
			{$durationString}
		</span>
	</div>
	<div class="text-2xl md:text-3xl text-amber-500 grid grid-cols-3">
		<span class="bg-blue-800">
			{$currentTrack?.bitDepth} bit
		</span>
		<span>&nbsp;</span>
		<span class="bg-blue-800">
			{$currentTrack?.samplingRate} kHz
		</span>
	</div>
</div>

<style lang="postcss">
	.marquee {
		animation-name: marquee;
		animation-duration: 15s;
		animation-iteration-count: infinite;
		animation-timing-function: linear;
	}

	@keyframes marquee {
		from {
			transform: translateX(0);
		}

		to {
			transform: translateX(-100%);
		}
	}
</style>
