<script>
	export let controls;

	import { playlistTracks, searchResults, artistAlbums } from '$lib/websocket';
	import { writable } from 'svelte/store';
	import ListItem from './ListItem.svelte';
	import ListAlbum from './ListAlbum.svelte';
	import Button from './Button.svelte';
	import List from './List.svelte';

	const searchTab = writable('albums');
	const artistName = writable('');
	const showArtistAlbums = writable(false);

	const playlistTitle = writable('');
	const showPlaylistTracks = writable(false);

	const onSubmit = (e) => {
		e.preventDefault();
		const formData = new FormData(e.target);

		if (formData.has('query')) {
			const query = formData.get('query');

			controls.search(query);
		}
	};
</script>

<div class="text-xl xl:text-4xl relative h-full px-4 flex flex-col">
	<form on:submit={onSubmit} class="flex flex-row pt-4">
		<input
			name="query"
			class="text-black p-2 rounded-none flex-grow"
			type="text"
			placeholder="Search"
		/>
		<Button type="submit">Search</Button>
	</form>
	<div class="text-xl xl:text-4xl my-4 gap-x-8 grid grid-cols-4">
		<button
			class:bg-amber-700={$searchTab !== 'albums'}
			class:bg-blue-500={$searchTab === 'albums'}
			on:click={() => searchTab.set('albums')}>Albums</button
		>
		<button
			class:bg-amber-700={$searchTab !== 'artists'}
			class:bg-blue-500={$searchTab === 'artists'}
			on:click={() => searchTab.set('artists')}>Artists</button
		>
		<button
			class:bg-amber-700={$searchTab !== 'tracks'}
			class:bg-blue-500={$searchTab === 'tracks'}
			on:click={() => searchTab.set('tracks')}>Tracks</button
		>
		<button
			class:bg-amber-700={$searchTab !== 'playlists'}
			class:bg-blue-500={$searchTab === 'playlists'}
			on:click={() => searchTab.set('playlists')}>Playlist</button
		>
	</div>
	<List>
		{#if $searchTab === 'albums'}
			{#each $searchResults.albums as album}
				<ListItem>
					<button
						class="w-full !text-left"
						on:click|stopPropagation={() => controls.playAlbum(album.id)}
					>
						<ListAlbum {album} />
					</button>
				</ListItem>
			{/each}
		{:else if $searchTab === 'artists'}
			{#each $searchResults.artists as artist}
				<ListItem>
					<button
						class="w-full !text-left"
						on:click|stopPropagation={() => {
							$artistAlbums.albums = [];
							$artistAlbums.id = null;
							artistName.set(artist.name);
							controls.fetchArtistAlbums(artist.id);
							showArtistAlbums.set(true);
						}}
					>
						<span>{artist.name}</span>
					</button>
				</ListItem>
			{/each}
		{:else if $searchTab === 'tracks'}
			{#each $searchResults.tracks as track}
				<ListItem>
					<button
						class="w-full !text-left"
						on:click|stopPropagation={() => controls.playTrack(track.id)}
					>
						<h3>{track.title}</h3>
						<h4 class="opacity-60">{track.artist.name}</h4>
					</button>
				</ListItem>
			{/each}
		{:else if $searchTab === 'playlists'}
			{#each $searchResults.playlists as playlist}
				<ListItem>
					<button
						class="w-full !text-left"
						on:click|stopPropagation={() => {
							$playlistTracks.tracks = [];
							$playlistTracks.id = null;
							playlistTitle.set(playlist.title);
							controls.fetchPlaylistTracks(playlist.id);
							showPlaylistTracks.set(true);
						}}
					>
						<span>{playlist.title}</span>
					</button>
				</ListItem>
			{/each}
		{/if}
	</List>

	{#if $showArtistAlbums}
		<div class="absolute w-full h-full flex flex-col bg-amber-950 top-0 left-0">
			<div class="flex flex-row justify-between py-4 bg-amber-900 px-4">
				<h2>albums by <span class="font-bold text-amber-500">{$artistName}</span></h2>
				<button on:click={() => showArtistAlbums.set(false)}>close</button>
			</div>
			<div class="overflow-y-scroll p-4">
				<List>
					{#each $artistAlbums.albums as album}
						<ListItem>
							<button
								class="w-full !text-left"
								on:click|stopPropagation={() => controls.playAlbum(album.id)}
							>
								<ListAlbum {album} />
							</button>
						</ListItem>
					{/each}
				</List>
			</div>
		</div>
	{/if}

	{#if $showPlaylistTracks}
		<div class="absolute w-full h-full flex flex-col bg-amber-950 top-0 left-0">
			<div class="flex flex-row items-center justify-between py-4 bg-amber-900 px-4">
				<h2>
					tracks in <span class="font-bold leading-none text-amber-500">{$playlistTitle}</span>
				</h2>
				<div class="text-2xl">
					<button class="bg-blue-900 p-2" on:click={() => controls.playPlaylist($playlistTracks.id)}
						>play</button
					>
					<button class="bg-blue-900 p-2" on:click={() => showPlaylistTracks.set(false)}
						>close</button
					>
				</div>
			</div>
			<div class="overflow-y-scroll p-4">
				<List>
					{#each $playlistTracks.tracks as track}
						<ListItem>
							<button
								class="w-full !text-left"
								on:click|stopPropagation={() => controls.playTrack(track.id)}
							>
								<h3>{track.title}</h3>
								<h4 class="opacity-60">{track.artist.name}</h4>
							</button>
						</ListItem>
					{/each}
				</List>
			</div>
		</div>
	{/if}
</div>
