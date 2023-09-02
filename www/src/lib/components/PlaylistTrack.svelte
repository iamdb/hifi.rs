<script>
	import { slide } from 'svelte/transition';

	export let controls, track, show;
</script>

<div class="flex flex-row items-center">
	<span class="flex-grow p-4 !text-left">
		<h3>{track.title}</h3>
		<h4 class="opacity-60">{track.artist.name}</h4>
	</span>
	{#if show}
		<span
			transition:slide={{ duration: 300, axis: 'x' }}
			class="flex flex-row items-center gap-x-8 px-4"
		>
			<button
				class="bg-amber-800 hover:bg-blue-800 px-2"
				on:click|stopPropagation={() => controls.playAlbum(track.album.id)}>Album</button
			>
			<span>or</span>
			<button
				class="bg-amber-800 hover:bg-blue-800 px-2"
				on:click|stopPropagation={() => controls.playTrack(track.id)}>Track</button
			>
		</span>
	{/if}
</div>
