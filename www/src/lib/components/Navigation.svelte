<script>
	import { Icon, QueueList, PlayCircle, MagnifyingGlass, Star } from 'svelte-hero-icons';

	export let setPage, activePage;
</script>

<nav
	class="flex w-full justify-evenly gap-2 p-safe *:flex *:h-[3.25rem] *:w-20 *:flex-col *:items-center *:overflow-visible *:text-nowrap *:px-4 *:py-1 *:text-[10px] *:font-medium *:transition-colors"
>
	<button
		class={$activePage === 'nowPlaying' ? 'text-blue-500' : 'text-gray-500'}
		on:click={() => setPage('nowPlaying')}
	>
		<Icon solid src={PlayCircle} />
		Now Playing
	</button>
	<button
		class={$activePage == 'queue' ? 'text-blue-500' : 'text-gray-500'}
		on:click={() => setPage('queue')}
	>
		<Icon solid src={QueueList} />
		Queue
	</button>
	<button
		class={$activePage == 'favorites' ? 'text-blue-500' : 'text-gray-500'}
		on:click={() => setPage('favorites')}
	>
		<Icon solid src={Star} />
		Favorites
	</button>
	<button
		class={$activePage == 'search' ? 'text-blue-500' : 'text-gray-500'}
		on:click={() => setPage('search')}
	>
		<Icon solid src={MagnifyingGlass} />
		Search
	</button>
</nav>
