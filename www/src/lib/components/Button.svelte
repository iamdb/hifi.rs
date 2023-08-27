<script>
	export let onClick;
</script>

<button
	class="bg-amber-900 hover:bg-yellow-950 text-gray-100 leading-none px-4 py-2 lg:px-6 lg:py-4 text-xl xl:text-3xl uppercase border-r-4 border-b-4 border-blue-800"
	on:click={onClick}
>
	<slot />
</button>
