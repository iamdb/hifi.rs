<script>
	import { onMount } from 'svelte';
	import { writable } from 'svelte/store';

	let ws;

	const isBuffering = writable(false);
	const position = writable(0);
	const status = writable('Stopped');
	const connected = writable(false);

	onMount(() => {
		ws = new WebSocket('ws://127.0.0.1:3000/ws');

		ws.onopen = () => {
			connected.set(true);
		};

		let retryInterval;

		ws.onclose = () => {
			retryInterval = setInterval(() => {
				ws.connected;
			});
		};

		ws.onmessage = (message) => {
			const json = JSON.parse(message.data);
			console.log(json);

			if (Object.hasOwn(json, 'buffering')) {
				isBuffering.set(json.buffering.is_buffering);
			}

			if (Object.hasOwn(json, 'position')) {
				position.set(json.position.clock);
			}

			if (Object.hasOwn(json, 'status')) {
				status.set(json.status.status);
			}
		};
	});

	const play = () => {
		ws.send(JSON.stringify({ playPause: null }));
	};
</script>

<h1>buffering: {$isBuffering}</h1>
<h1>position: {Math.floor($position / 1000 / 1000 / 1000)}</h1>
<h1>status: {$status}</h1>

<button on:click={play}>Play/Pause</button>
