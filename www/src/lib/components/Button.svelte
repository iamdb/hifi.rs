<script>
	export let onClick;
</script>

<button
	class="bg-amber-900 hover:bg-yellow-950 text-gray-200 leading-none font-bold px-6 py-4 text-md xl:text-2xl uppercase border-r-4 border-b-4 border-blue-800"
	on:click={onClick}
>
	<slot />
</button>
