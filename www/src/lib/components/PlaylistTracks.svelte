<script>
	import { playlistTracks, playlistTitle } from '$lib/websocket';
	import List from './List.svelte';
	import ListItem from './ListItem.svelte';
	import PlaylistTrack from './PlaylistTrack.svelte';
	import { writable } from 'svelte/store';

	export let controls, showPlaylistTracks;

	const show = writable(null);

	const toggle = (id) => {
		if ($show === id) {
			show.set(null);
		} else {
			show.set(id);
		}
	};
</script>

<div class="absolute w-full h-full flex flex-col bg-amber-950 top-0 left-0">
	<div class="flex flex-row justify-between items-center py-4 bg-amber-900 px-4">
		<h2 class="text-2xl xl:text-4xl pr-4">
			tracks in <span class="font-bold leading-none text-amber-500">{$playlistTitle}</span>
		</h2>
		<div class="text-lg xl:text-2xl flex flex-row flex-nowrap gap-x-2">
			<button
				class="bg-blue-800 hover:bg-amber-800 p-2"
				on:click={() => {
					show.set(null);
					controls.playPlaylist($playlistTracks.id);
				}}>play</button
			>
			<button
				class="bg-blue-800 hover:bg-amber-800 p-2"
				on:click={() => {
					showPlaylistTracks.set(false);
					show.set(null);
				}}>close</button
			>
		</div>
	</div>
	<div class="overflow-y-scroll p-2 lg:p-4">
		<List>
			{#each $playlistTracks.tracks as track}
				<ListItem>
					<button class="w-full" on:click={() => toggle(track.id)}>
						<PlaylistTrack {controls} {track} show={$show === track.id} />
					</button>
				</ListItem>
			{/each}
		</List>
	</div>
</div>
