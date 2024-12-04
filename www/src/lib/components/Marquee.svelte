<script lang="ts">
	import { afterUpdate } from 'svelte';
	import { writable } from 'svelte/store';

	export let input: string;

	let titleWidth: number, titleWrapperWidth: number;

	const enableMarquee = writable(false);

	afterUpdate(() => {
		if (titleWidth > titleWrapperWidth) {
			enableMarquee.set(true);
		} else {
			enableMarquee.set(false);
		}
	});
</script>

<div bind:offsetWidth={titleWrapperWidth} class="flex overflow-hidden">
	<span class:marquee={$enableMarquee} bind:offsetWidth={titleWidth} class="whitespace-nowrap">
		{input}
	</span>

	{#if $enableMarquee}
		<span class="whitespace-nowrap marquee extra-element">
			{input}
		</span>
	{/if}
</div>

<style lang="postcss">
	@media (prefers-reduced-motion) {
		.marquee {
			overflow: hidden;
			text-overflow: ellipsis;
			white-space: nowrap;
		}
		.extra-element {
			display: none;
		}
	}

	@media not (prefers-reduced-motion) {
		.marquee {
			padding-right: 2rem;
			animation-name: marquee;
			animation-duration: 15s;
			animation-iteration-count: infinite;
			animation-timing-function: cubic-bezier(0.3, 0, 0.7, 1);
		}

		@keyframes marquee {
			0% {
				transform: translateX(0%);
			}

			20% {
				transform: translateX(0%);
			}

			100% {
				transform: translateX(-100%);
			}
		}
	}
</style>
