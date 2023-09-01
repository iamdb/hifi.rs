<script>
	export let src, alt;
</script>

<div
	class="aspect-video w-full md:w-1/2 md:aspect-square relative bg-amber-900 p-2 2xl:p-8 flex-shrink-0 mx-auto flex items-center justify-center"
>
	<div
		class="w-full h-full flex flex-row flex-wrap items-center justify-center p-2 overflow-hidden 2xl:p-8 mix-blend-soft-light opacity-75 absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2"
	>
		<img class="block w-full h-full max-w-full object-cover" {src} {alt} />
	</div>
	<div
		class="w-full h-full backdrop-hue-rotate-30 backdrop-contrast-75 overflow-hidden backdrop-blur-sm flex flex-row flex-wrap items-center justify-center"
	>
		<img
			{src}
			{alt}
			class="block w-full h-full md:h-auto object-contain md:w-auto max-w-full relative z-10"
		/>
	</div>
</div>
