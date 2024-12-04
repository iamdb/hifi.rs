<script>
	import { searchResults, playlistTracks, artistAlbums, playlistTitle } from '$lib/websocket';
	import { writable } from 'svelte/store';
	import ListItem from './ListItem.svelte';
	import ListAlbum from './ListAlbum.svelte';
	import Button from './Button.svelte';
	import List from './List.svelte';
	import PlaylistTracks from './PlaylistTracks.svelte';
	import { Icon, MagnifyingGlass, XMark } from 'svelte-hero-icons';
	import ListTrack from './ListTrack.svelte';
	import { onMount } from 'svelte';

	export let controls;

	const searchTab = writable('albums');
	const artistName = writable('');
	const showArtistAlbums = writable(false);

	const showPlaylistTracks = writable(false);

	const onSubmit = (e) => {
		e.preventDefault();
		const formData = new FormData(e.target);

		if (formData.has('query')) {
			const query = formData.get('query');

			controls.search(query);
		}
	};

	let searchInput;

	onMount(() => searchInput.focus());
</script>

<div class="flex flex-col flex-grow gap-4 max-h-full">
	<div class="flex flex-col gap-4 p-4">
		<form on:submit={onSubmit} class="flex flex-row">
			<input
				bind:this={searchInput}
				name="query"
				class="p-2 w-full text-black rounded"
				type="text"
				placeholder="Search"
				spellcheck="false"
				autocomplete="off"
				autocorrect="off"
				autocapitalize="off"
			/>
			<Button type="submit"><Icon src={MagnifyingGlass} class="size-6" solid /></Button>
		</form>

		<div class="flex justify-between *:transition-colors *:px-2 *:py-1 *:rounded-full">
			<button class:bg-blue-800={$searchTab === 'albums'} on:click={() => searchTab.set('albums')}>
				Albums
			</button>
			<button
				class:bg-blue-800={$searchTab === 'artists'}
				on:click={() => searchTab.set('artists')}
			>
				Artists
			</button>
			<button class:bg-blue-800={$searchTab === 'tracks'} on:click={() => searchTab.set('tracks')}>
				Tracks
			</button>
			<button
				class:bg-blue-800={$searchTab === 'playlists'}
				on:click={() => searchTab.set('playlists')}
			>
				Playlist
			</button>
		</div>
	</div>
	<List>
		{#if $searchTab === 'albums'}
			{#each $searchResults.albums as album}
				<ListItem>
					<button
						class="p-4 w-full text-left"
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
						class="p-4 w-full text-lg text-left truncate"
						on:click|stopPropagation={() => {
							$artistAlbums.albums = [];
							$artistAlbums.id = null;
							artistName.set(artist.name);
							controls.fetchArtistAlbums(artist.id);
							showArtistAlbums.set(true);
						}}
					>
						{artist.name}
					</button>
				</ListItem>
			{/each}
		{:else if $searchTab === 'tracks'}
			{#each $searchResults.tracks as track}
				<ListItem>
					<button
						class="p-4 w-full text-left"
						on:click|stopPropagation={() => controls.playTrack(track.id)}
					>
						<ListTrack {track} />
					</button>
				</ListItem>
			{/each}
		{:else if $searchTab === 'playlists'}
			{#each $searchResults.playlists as playlist}
				<ListItem>
					<button
						class="p-4 w-full text-lg text-left truncate"
						on:click|stopPropagation={() => {
							$playlistTracks.tracks = [];
							$playlistTracks.id = null;
							playlistTitle.set(playlist.title);
							controls.fetchPlaylistTracks(playlist.id);
							showPlaylistTracks.set(true);
						}}
					>
						{playlist.title}
					</button>
				</ListItem>
			{/each}
		{/if}
	</List>

	{#if $showArtistAlbums}
		<div class="flex absolute top-0 left-0 flex-col w-full h-full bg-black">
			<div class="flex flex-row justify-between py-4 px-4 bg-black">
				<h2>Albums by <span class="font-bold">{$artistName}</span></h2>
				<button on:click={() => showArtistAlbums.set(false)}
					><Icon src={XMark} class="size-6" /></button
				>
			</div>
			<div class="overflow-y-scroll">
				<List>
					{#each $artistAlbums.albums as album}
						<ListItem>
							<button
								class="p-4 w-full text-left"
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
		<PlaylistTracks {controls} {showPlaylistTracks} />
	{/if}
</div>
