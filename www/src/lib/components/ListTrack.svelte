<script>
	import Info from './Info.svelte';

	export let track;
</script>

<div>
	<div class="flex justify-between items-center">
		<h3 class="text-lg truncate">
			{track.title}
		</h3>
		<Info explicit={track.explicit} hiresAvailable={track.hiresAvailable} />
	</div>

	<h4 class="text-gray-400 truncate">
		{track.title}
	</h4>
</div>
