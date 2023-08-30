<script>
	import { afterUpdate } from 'svelte';
	import Button from './Button.svelte';
	import { currentStatus } from '$lib/websocket';

	export let showList, toggleList, controls, navHeight;

	let height;

	afterUpdate(() => {
		navHeight.set(height);
	});
</script>

<div
	bind:offsetHeight={height}
	class="flex flex-row gap-x-4 md:mt-0 px-4 md:px-12 items-end justify-between"
>
	<span class:fixed-button={$showList}>
		<Button onClick={toggleList}>{$showList ? 'Close' : 'Menu'}</Button>
	</span>
	<div class="flex flex-row justify-end gap-x-4 flex-grow">
		<Button onClick={() => controls?.previous()}>Previous</Button>
		<Button onClick={() => controls?.playPause()}>
			{#if $currentStatus === 'Playing'}
				Pause
			{:else}
				Play
			{/if}
		</Button>
		<Button onClick={() => controls?.next()}>Next</Button>
	</div>
</div>

<style lang="postcss">
	.fixed-button {
		@apply fixed z-20 bottom-4 left-4 md:z-auto md:relative md:bottom-auto md:left-auto;
	}
</style>
