<script>
	export let album;
</script>

<h3 class="text-left">
	{album.title}
	<span class="text-2xl align-middle">({album.releaseYear})</span>
</h3>

<h4 class="opacity-60 text-left">
	{album.artist.name}
	{#if album.explicit}
		<sup>e</sup>
	{/if}
	{#if album.hiresAvailable}
		<sup>h</sup>
	{/if}
</h4>
