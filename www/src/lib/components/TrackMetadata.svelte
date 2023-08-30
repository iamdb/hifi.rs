<script>
	import { currentTrack, positionString, durationString } from '$lib/websocket';
</script>

<span>{$currentTrack?.artistName || ''}</span>

<span class="font-semibold py-2 md:py-4 xl:py-8 bg-amber-900 leading-[1.15em] px-2 md:px-4 xl:px-8">
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

<div class="flex flex-col gap-y-2 max-w-xs mx-auto">
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
