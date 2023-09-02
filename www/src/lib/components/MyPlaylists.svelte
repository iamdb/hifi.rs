<script>
	import { userPlaylists, playlistTracks, playlistTitle } from '$lib/websocket';
	import { writable } from 'svelte/store';
	import List from './List.svelte';
	import ListItem from './ListItem.svelte';
	import PlaylistTracks from './PlaylistTracks.svelte';

	export let controls;

	const showPlaylistTracks = writable(false);
</script>

<div class="h-full p-4 relative flex flex-col">
	<List>
		{#each $userPlaylists as playlist}
			<ListItem>
				<button
					on:click|stopPropagation={() => {
						$playlistTracks.tracks = [];
						$playlistTracks.id = null;
						playlistTitle.set(playlist.title);
						controls.fetchPlaylistTracks(playlist.id);
						showPlaylistTracks.set(true);
					}}
					class="w-full py-4"
				>
					{playlist.title}
				</button>
			</ListItem>
		{/each}
	</List>

	{#if $showPlaylistTracks}
		<PlaylistTracks {controls} {showPlaylistTracks} />
	{/if}
</div>
