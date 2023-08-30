<script>
	import { fly } from 'svelte/transition';
	import { quintOut } from 'svelte/easing';
	import { queue, entityTitle } from '$lib/websocket';

	export let showList, navHeight, controls;
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
		<div
			class="flex flex-row gap-x-8 mb-8 py-1 px-2 justify-center items-center text-center text-xl xl:text-4xl"
		>
			<p class="font-bold">{$entityTitle}</p>
		</div>
		<ul
			class="text-2xl xl:text-4xl gap-y-2 flex flex-col px-4 md:px-4 xl:px-12 lg:px-24 leading-tight overflow-y-scroll"
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
						<span>{track.number.toString().padStart(2, '0')}</span>
						<span>{track.title}</span>
					</button>
				</li>
			{/each}
		</ul>
	</div>
{/if}
