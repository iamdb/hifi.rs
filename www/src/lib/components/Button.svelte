<script>
	export let onClick = null,
		type = null;
</script>

<button
	{type}
	class="bg-amber-900 hover:bg-blue-800 text-gray-100 leading-none px-4 py-2 lg:px-6 lg:py-4 text-2xl xl:text-3xl uppercase border-r-4 border-b-4 border-blue-800"
	on:click|stopPropagation={onClick}
>
	<slot />
</button>
