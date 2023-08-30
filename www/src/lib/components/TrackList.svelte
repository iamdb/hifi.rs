<script>
	import { fly } from 'svelte/transition';
	import { quintOut } from 'svelte/easing';
	import { queue, entityTitle, searchResults } from '$lib/websocket';
	import { writable } from 'svelte/store';

	export let showList, navHeight, controls;

	const tab = writable('nowPlaying');
	const searchTab = writable('albums');
</script>

{#if $showList}
	<div
		on:click|stopPropagation={() => {}}
		on:keyup|stopPropagation={() => {}}
		role="menu"
		tabindex="0"
		style:padding-bottom={`${$navHeight + 32}px`}
		transition:fly={{ duration: 300, easing: quintOut, x: '100%' }}
		class="fixed h-[100dvh] md:!pb-0 md:h-full md:absolute z-10 flex flex-col backdrop-blur-sm bg-opacity-90 w-full text-left top-0 right-0 bg-amber-950"
	>
		<div class="text-xl xl:text-4xl grid grid-cols-2">
			<button class:bg-blue-500={$tab === 'nowPlaying'} on:click={() => tab.set('nowPlaying')}
				>Now Playing</button
			>
			<button class:bg-blue-500={$tab === 'search'} on:click={() => tab.set('search')}
				>Search</button
			>
		</div>

		<div class="py-4 overflow-hidden flex-grow">
			{#if $tab === 'nowPlaying'}
				<div class="h-full flex flex-col">
					<div class="mb-4 px-2 text-center text-xl xl:text-4xl">
						<p class="font-bold">{$entityTitle}</p>
					</div>
					<ul
						class="text-2xl xl:text-4xl gap-y-2 flex flex-col px-4 md:px-4 xl:px-12 lg:px-20 leading-tight overflow-y-scroll"
					>
						{#each $queue as track}
							<li
								class:opacity-60={track.status === 'Played'}
								class:text-amber-500={track.status === 'Playing'}
							>
								<button
									on:click|stopPropagation={() => controls.skipTo(track.number)}
									class="grid grid-flow-col-dense gap-x-4"
								>
									<span>{track.position.toString().padStart(2, '0')}</span>
									<span>{track.title}</span>
								</button>
							</li>
						{/each}
					</ul>
				</div>
			{:else if $tab === 'search'}
				<div class="text-xl xl:text-4xl h-full flex mb-4 flex-col items-center">
					<form>
						<input class="text-black p-2 rounded-none" type="text" placeholder="Search" />
						<button type="submit">Search</button>
					</form>
					<div class="text-xl xl:text-4xl my-2 gap-x-8 grid grid-cols-4">
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
					<ul
						class="text-2xl w-full xl:text-4xl flex flex-col gap-y-4 px-2 md:px-4 xl:px-12 lg:px-24 leading-tight overflow-y-scroll"
					>
						{#if $searchTab === 'albums'}
							{#each $searchResults.albums.items as album}
								<li>
									<button
										class="flex flex-col hover:bg-amber-500/25 px-4 w-full text-left"
										on:click|stopPropagation={() => controls.playAlbum(album.id)}
									>
										<span>{album.title}</span>
										<span class="opacity-60">{album.artist.name}</span>
									</button>
								</li>
							{/each}
						{:else if $searchTab === 'artists'}
							{#each $searchResults.artists.items as artist}
								<li>
									<button
										class="text-left hover:bg-amber-500/25 w-full px-4"
										on:click|stopPropagation={() => {}}
									>
										<span>{artist.name}</span>
									</button>
								</li>
							{/each}
						{:else if $searchTab === 'tracks'}
							{#each $searchResults.tracks.items as track}
								<li>
									<button
										class="text-left flex flex-col hover:bg-amber-500/25 w-full px-4"
										on:click|stopPropagation={() => controls.playTrack(track.id)}
									>
										<span>{track.title}</span>
										<span class="opacity-60">{track.performer?.name}</span>
									</button>
								</li>
							{/each}
						{:else if $searchTab === 'playlists'}
							{#each $searchResults.playlists.items as playlist}
								<li>
									<button
										class="text-left hover:bg-amber-500/25 w-full px-4"
										on:click|stopPropagation={() => controls.playPlaylist(playlist.id)}
									>
										<span>{playlist.name}</span>
									</button>
								</li>
							{/each}
						{/if}
					</ul>
				</div>
			{/if}
		</div>
	</div>
{/if}
