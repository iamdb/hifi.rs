<script>
	import { playlistTracks, playlistTitle } from '$lib/websocket';
	import List from './List.svelte';
	import ListItem from './ListItem.svelte';
	import ListTrack from './ListTrack.svelte';
	import { XMark, Icon, Play } from 'svelte-hero-icons';

	export let controls, showPlaylistTracks;
</script>

<div class="flex absolute top-0 left-0 flex-col w-full h-full bg-black">
	<div class="flex flex-row justify-between py-4 px-4 bg-black">
		<h2>
			Tracks in <span class="font-bold">{$playlistTitle}</span>
		</h2>
		<div class="flex flex-row flex-nowrap gap-x-2">
			<button
				on:click={() => {
					controls.playPlaylist($playlistTracks.id);
				}}
			>
				<Icon src={Play} class="size-6" />
			</button>
			<button
				on:click={() => {
					showPlaylistTracks.set(false);
				}}
			>
				<Icon src={XMark} class="size-6" />
			</button>
		</div>
	</div>
	<div class="overflow-y-scroll">
		<List>
			{#each $playlistTracks.tracks as track}
				<ListItem>
					<button class="p-4 w-full text-left" on:click={() => controls.playTrack(track.id)}>
						<ListTrack {controls} {track} />
					</button>
				</ListItem>
			{/each}
		</List>
	</div>
</div>
