<script>
	export let onClick;
</script>

<button
	class="bg-amber-900 hover:bg-yellow-950 text-gray-200 font-bold px-6 py-4 text-2xl uppercase border-r-2 border-b"
	on:click={onClick}
>
	<slot />
</button>
