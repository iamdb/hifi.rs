<script>
	export let onClick = null,
		type = null;
</script>

<button {type} class="py-2 px-4 text-4xl leading-none" on:click|stopPropagation={onClick}>
	<slot />
</button>
