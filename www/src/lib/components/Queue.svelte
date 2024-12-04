<script>
	import { queue, entityTitle, listType, currentTrack } from '$lib/websocket';
	import Info from './Info.svelte';
	import List from './List.svelte';
	import ListItem from './ListItem.svelte';

	export let controls;
</script>

<div class="flex flex-col flex-grow gap-4 max-h-full">
	<div class="p-4 text-center">
		<p class="text-lg">{$entityTitle}</p>
		{#if $listType === 'Album'}
			<p class="text-lg">by {$currentTrack.artist.name}</p>
		{/if}
	</div>

	<List>
		{#each $queue as track}
			<ListItem>
				<button
					class:text-gray-500={track.status === 'Played'}
					class:bg-blue-800={track.status === 'Playing'}
					on:click|stopPropagation={() => controls.skipTo(track.position)}
					class="flex flex-row gap-x-4 p-4 w-full text-base text-left"
				>
					{#if $listType === 'Album' || $listType === 'Track'}
						<span class="self-start">{track.number.toString().padStart(2, '0')}</span>
					{:else if $listType === 'Playlist'}
						<span>{track.position.toString().padStart(2, '0')}</span>
					{/if}
					<div class="flex overflow-hidden flex-grow justify-between items-center">
						<span class="truncate">
							{track.title}
						</span>
						<Info explicit={track.explicit} hiresAvailable={track.hiresAvailable} />
					</div>
				</button>
			</ListItem>
		{/each}
	</List>
</div>
