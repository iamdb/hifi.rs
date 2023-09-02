<script>
	import { queue, entityTitle, listType, currentTrack, secsToTimecode } from '$lib/websocket';
	import List from './List.svelte';
	import ListItem from './ListItem.svelte';

	export let controls;
</script>

<div class="h-full p-2 lg:p-4 flex flex-col">
	<div class="mb-4 text-center">
		<p class="text-2xl xl:text-4xl">{$entityTitle}</p>
		{#if $listType === 'Album'}
			<p class="text-xl xl:text-3xl opacity-60">by {$currentTrack.artist.name}</p>
		{/if}
	</div>
	<List>
		{#each $queue as track}
			<ListItem>
				<button
					class:opacity-60={track.status === 'Played'}
					class:text-amber-500={track.status === 'Playing'}
					on:click|stopPropagation={() => controls.skipTo(track.position)}
					class="flex flex-row text-left gap-x-4 p-4 w-full"
				>
					{#if $listType === 'Album' || $listType === 'Track'}
						<span class="self-start">{track.number.toString().padStart(2, '0')}</span>
					{:else if $listType === 'Playlist'}
						<span>{track.position.toString().padStart(2, '0')}</span>
					{/if}
					<span>
						{track.title}
						<span class="text-2xl opacity-60">{secsToTimecode(track.durationSeconds)}</span>
					</span>
				</button>
			</ListItem>
		{/each}
	</List>
</div>
