<script>
	import { fly } from 'svelte/transition';
	import { quintOut } from 'svelte/easing';
	import { writable } from 'svelte/store';
	import Search from './Search.svelte';
	import NowPlaying from './NowPlaying.svelte';
	import MyPlaylists from './MyPlaylists.svelte';

	export let showList, navHeight, controls;

	const tab = writable('nowPlaying');
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
		<div class="text-2xl xl:text-4xl grid grid-cols-3 bg-blue-950">
			<button
				class="py-2"
				class:bg-blue-800={$tab === 'nowPlaying'}
				on:click={() => tab.set('nowPlaying')}>Now Playing</button
			>
			<button class:bg-blue-800={$tab === 'myPlaylists'} on:click={() => tab.set('myPlaylists')}>
				My Playlists
			</button>
			<button class:bg-blue-800={$tab === 'search'} on:click={() => tab.set('search')}
				>Search</button
			>
		</div>

		<div class="overflow-hidden flex-grow">
			{#if $tab === 'nowPlaying'}
				<NowPlaying {controls} />
			{:else if $tab === 'search'}
				<Search {controls} />
			{:else if $tab === 'myPlaylists'}
				<MyPlaylists {controls} />
			{/if}
		</div>
	</div>
{/if}
