<script lang="ts">
	import { onMount } from 'svelte';

	import Clock from 'virtual:icons/ic/round-access-time-filled';
	import People from 'virtual:icons/ic/baseline-people';

	const WEBSOCKET_URL = 'ws://127.0.0.1:2325';

	const CANVAS_WIDTH = 1024;
	const CANVAS_HEIGHT = 1024;
	const INITIAL_SCALE = 0.5;
	const MIN_SCALE = 0.1;
	const MAX_SCALE = 5.0;

	// state stuff
	let canvas: HTMLCanvasElement;
	let ctx: CanvasRenderingContext2D;
	let ws: WebSocket;
	let isConnected = $state(false);
	let isPainting = $state(false);
	let selectedColor = $state('#000000');

	// stats state
	let clientCount = $state(0);
	let requestsPerSecond = $state(0.0);

	// canvas transformation state
	let scale = $state(INITIAL_SCALE);
	let offsetX = $state(0);
	let offsetY = $state(0);
	let isDragging = $state(false);
	let lastMouseX = 0;
	let lastMouseY = 0;
	let canvasContainer: HTMLDivElement;

	// pixel data
	let pixels: Uint32Array = new Uint32Array(CANVAS_WIDTH * CANVAS_HEIGHT);

	// === message types ===
	const MessageType = {
		SET_PIXEL: 1,
		GET_PIXEL: 2,
		GET_ALL_PIXELS: 3,
		GET_STATS: 4
	};

	const ResponseType = {
		PIXEL_COLOR: 10,
		ERROR: 11,
		GET_ALL_PIXELS: 12,
		STATS: 13
	};

	// === packing/unpacking helpers ===
	function packCoordinates(x: number, y: number): Uint8Array {
		const packed = (x << 10) | y;
		return new Uint8Array([(packed >> 16) & 0xff, (packed >> 8) & 0xff, packed & 0xff]);
	}

	function unpackCoordinates(bytes: Uint8Array): [number, number] {
		const packed = (bytes[0] << 16) | (bytes[1] << 8) | bytes[2];
		const x = (packed >> 10) & 0x3ff;
		const y = packed & 0x3ff;
		return [x, y];
	}

	function packRGB(color: number): Uint8Array {
		return new Uint8Array([(color >> 16) & 0xff, (color >> 8) & 0xff, color & 0xff]);
	}

	function unpackRGB(bytes: Uint8Array): number {
		return (bytes[0] << 16) | (bytes[1] << 8) | bytes[2];
	}

	// === color conversion helpers ===
	function hexToRGB(hex: string): number {
		const r = parseInt(hex.slice(1, 3), 16);
		const g = parseInt(hex.slice(3, 5), 16);
		const b = parseInt(hex.slice(5, 7), 16);
		return (r << 16) | (g << 8) | b;
	}

	function rgbToHex(rgb: number): string {
		return '#' + rgb.toString(16).padStart(6, '0');
	}

	// === websocket stuff ===
	function connectWebSocket() {
		try {
			ws = new WebSocket(WEBSOCKET_URL);

			ws.onopen = () => {
				console.log('Connected to WebSocket');
				isConnected = true;
				requestAllPixels();
			};

			ws.onclose = () => {
				console.log('WebSocket connection closed');
				isConnected = false;
				// try to reconnect after 3 seconds
				setTimeout(connectWebSocket, 3000);
			};

			ws.onerror = (error) => {
				console.error('WebSocket error:', error);
				isConnected = false;
			};

			ws.onmessage = (event) => {
				if (event.data instanceof Blob) {
					event.data.arrayBuffer().then((buffer) => {
						handleMessage(new Uint8Array(buffer));
					});
				}
			};
		} catch (error) {
			console.error('Failed to connect to WebSocket:', error);
			isConnected = false;
		}
	}

	function handleMessage(data: Uint8Array) {
		if (data.length === 0) return;

		const messageType = data[0];

		switch (messageType) {
			case MessageType.SET_PIXEL:
				// Parse set pixel response: [type:1][coord:3][color:3] = 7 bytes
				if (data.length >= 7) {
					const coordBytes = data.slice(1, 4);
					const rgbBytes = data.slice(4, 7);
					const [x, y] = unpackCoordinates(coordBytes);
					const color = unpackRGB(rgbBytes);
					setPixelLocal(x, y, color);
				}
				break;

			case ResponseType.GET_ALL_PIXELS:
				// Parse all pixels response: [type:1][pixels:3*n] = 1 + 3*n bytes
				if (data.length >= 4) {
					const pixelData = data.slice(1);

					// 3 bytes per pixel (RGB)
					for (let i = 0; i < pixelData.length; i += 3) {
						if (i + 2 < pixelData.length) {
							const pixelIndex = i / 3;
							const rgbBytes = pixelData.slice(i, i + 3);
							const color = unpackRGB(rgbBytes);
							pixels[pixelIndex] = color;
						}
					}
					renderCanvas();

					// if the canvas is still hidden, show it
					if (canvas.style.display !== 'block') {
						canvas.style.display = 'block';
					}
				}
				break;

			case ResponseType.PIXEL_COLOR:
				// Parse pixel color response: [type:1][color:3] = 4 bytes
				if (data.length >= 4) {
					const rgbBytes = data.slice(1, 4);
					const color = unpackRGB(rgbBytes);
					console.log('Pixel color:', rgbToHex(color));
				}
				break;

			case ResponseType.ERROR:
				// Parse error response: [type:1][error_code:1] = 2 bytes
				if (data.length >= 2) {
					const errorCode = data[1];
					console.error('Server error:', errorCode);
				}
				break;

			case ResponseType.STATS:
				// Parse stats message: [type:1][client_count:4][rps:4] = 9 bytes
				if (data.length >= 9) {
					const clientCountBytes = data.slice(1, 5);
					const rpsBytes = data.slice(5, 9);

					// convert bytes to numbers
					const clientCountView = new DataView(
						clientCountBytes.buffer,
						clientCountBytes.byteOffset
					);
					const rpsView = new DataView(rpsBytes.buffer, rpsBytes.byteOffset);

					// using big-endian
					clientCount = clientCountView.getUint32(0, false);
					requestsPerSecond = rpsView.getFloat32(0, false);

					console.log(`Stats - Clients: ${clientCount}, RPS: ${requestsPerSecond.toFixed(2)}`);
				}
				break;
		}
	}

	function sendSetPixel(x: number, y: number, color: number) {
		if (!isConnected || !ws) return;

		const message = new Uint8Array(7);
		message[0] = MessageType.SET_PIXEL;

		const coords = packCoordinates(x - 1, y - 1);
		message.set(coords, 1);

		const rgb = packRGB(color);
		message.set(rgb, 4);

		ws.send(message);
	}

	function requestAllPixels() {
		if (!isConnected || !ws) return;

		const message = new Uint8Array(1);
		message[0] = MessageType.GET_ALL_PIXELS;
		ws.send(message);
	}

	// === mouse handlers ===
	function getCanvasCoordinates(event: MouseEvent): [number, number] {
		const rect = canvas.getBoundingClientRect();
		const clientX = event.clientX - rect.left;
		const clientY = event.clientY - rect.top;

		// Convert from screen coordinates to canvas coordinates
		const canvasX = Math.floor(clientX / scale);
		const canvasY = Math.floor(clientY / scale);

		return [canvasX, canvasY];
	}

	function handleMouseDown(event: MouseEvent) {
		if (event.button === 0) {
			// left
			if ((event.target as HTMLElement) !== canvas) return; // don't start painting if not on canvas
			isPainting = true;
			const [x, y] = getCanvasCoordinates(event);
			if (x >= 0 && x <= CANVAS_WIDTH && y >= 0 && y <= CANVAS_HEIGHT) {
				const color = hexToRGB(selectedColor);
				sendSetPixel(x, y, color);
			}
		} else if (event.buttons === 2 || event.buttons === 4) {
			// right/middle
			isDragging = true;
			lastMouseX = event.clientX;
			lastMouseY = event.clientY;
		}
	}

	function handleMouseMove(event: MouseEvent) {
		if (isPainting && event.buttons === 1) {
			// left
			const [x, y] = getCanvasCoordinates(event);
			if (x >= 0 && x <= CANVAS_WIDTH && y >= 0 && y <= CANVAS_HEIGHT) {
				const color = hexToRGB(selectedColor);
				sendSetPixel(x, y, color);
			}
		} else if (isDragging && (event.buttons === 2 || event.buttons === 4)) {
			// right/middle
			const deltaX = event.clientX - lastMouseX;
			const deltaY = event.clientY - lastMouseY;

			offsetX += deltaX;
			offsetY += deltaY;

			lastMouseX = event.clientX;
			lastMouseY = event.clientY;

			updateCanvasPosition();
		}
	}

	function handleMouseUp(event: MouseEvent) {
		if (event.button === 0) {
			// left
			isPainting = false;
		} else if (event.buttons === 2 || event.buttons === 4) {
			// right/middle
			isDragging = false;
		}
	}

	function handleWheel(event: WheelEvent) {
		event.preventDefault();

		const rect = canvas.getBoundingClientRect();
		const mouseX = event.clientX - rect.left;
		const mouseY = event.clientY - rect.top;

		// get canvas position before zoom
		const canvasX = mouseX / scale;
		const canvasY = mouseY / scale;

		// update the scale
		const zoomFactor = event.deltaY > 0 ? 0.9 : 1.1;
		const newScale = Math.max(MIN_SCALE, Math.min(MAX_SCALE, scale * zoomFactor));

		if (newScale !== scale) {
			scale = newScale;

			// adjust the offset
			const newMouseX = canvasX * scale;
			const newMouseY = canvasY * scale;

			offsetX += mouseX - newMouseX;
			offsetY += mouseY - newMouseY;

			updateCanvasPosition();
		}
	}

	// === localstorage helpers ===
	function saveState() {
		if (!canvas) return;

		const state = {
			scale,
			offsetX,
			offsetY,
			selectedColor
		};
		localStorage.setItem('settings', JSON.stringify(state));
	}

	function loadState() {
		const state = localStorage.getItem('settings');
		if (state) {
			const {
				scale: savedScale,
				offsetX: savedOffsetX,
				offsetY: savedOffsetY,
				selectedColor: savedColor
			} = JSON.parse(state);
			scale = savedScale || INITIAL_SCALE;
			offsetX = savedOffsetX || 0;
			offsetY = savedOffsetY || 0;
			selectedColor = savedColor || `#${getRandomHexColor()}`;
		}
		updateCanvasPosition();
	}

	// === canvas helpers ===
	function setPixelLocal(x: number, y: number, color: number) {
		if (x >= 0 && x <= CANVAS_WIDTH && y >= 0 && y <= CANVAS_HEIGHT) {
			pixels[y * CANVAS_WIDTH + x] = color;
			renderPixel(x, y, color);
		}
	}

	function renderPixel(x: number, y: number, color: number) {
		if (!ctx) return;

		const r = (color >> 16) & 0xff;
		const g = (color >> 8) & 0xff;
		const b = color & 0xff;

		ctx.fillStyle = `rgb(${r}, ${g}, ${b})`;
		ctx.fillRect(x, y, 1, 1);
	}

	function renderCanvas() {
		if (!ctx) return;

		const imageData = ctx.createImageData(CANVAS_WIDTH, CANVAS_HEIGHT);
		const data = imageData.data;

		for (let i = 0; i < pixels.length; i++) {
			const color = pixels[i];
			const pixelIndex = i * 4;

			data[pixelIndex] = (color >> 16) & 0xff; // Red
			data[pixelIndex + 1] = (color >> 8) & 0xff; // Green
			data[pixelIndex + 2] = color & 0xff; // Blue
			data[pixelIndex + 3] = 255; // Alpha
		}

		ctx.putImageData(imageData, 0, 0);
	}

	function getRandomHexColor(): string {
		return Math.floor(Math.random() * 0xffffff)
			.toString(16)
			.padStart(6, '0');
	}

	function updateCanvasPosition() {
		if (!canvas) return;

		canvas.style.transform = `translate(${offsetX}px, ${offsetY}px) scale(${scale})`;
	}

	function resetView() {
		scale = INITIAL_SCALE;
		offsetX = 0;
		offsetY = 0;
		updateCanvasPosition();
		centerView();
	}

	function centerView() {
		if (!canvasContainer || !canvas) return;

		const containerRect = canvasContainer.getBoundingClientRect();
		const canvasWidth = CANVAS_WIDTH * scale;
		const canvasHeight = CANVAS_HEIGHT * scale;

		offsetX = (containerRect.width - canvasWidth) / 2;
		offsetY = (containerRect.height - canvasHeight) / 2;
		updateCanvasPosition();
	}

	onMount(() => {
		ctx = canvas.getContext('2d')!;
		ctx.imageSmoothingEnabled = false;

		// initialize the canvas
		canvas.width = CANVAS_WIDTH;
		canvas.height = CANVAS_HEIGHT;

		// clear canvas to white
		ctx.fillStyle = '#ffffff';
		ctx.fillRect(0, 0, CANVAS_WIDTH, CANVAS_HEIGHT);

		// center the canvas
		centerView();

		// connect to the websocket
		connectWebSocket();

		// prevent context menu opening on right click
		canvas.addEventListener('contextmenu', (e) => e.preventDefault());

		// load saved state
		loadState();

		// save state on unload
		window.addEventListener('beforeunload', () => {
			saveState();
		});

		// close the websocket
		if (ws) {
			ws.close();
		}
	});

	$effect(() => {
		if (!isConnected) {
			// unhide the canvas when disconnected to avoid a blank screen
			if (canvas) {
				canvas.style.display = 'block';
			}
		}
	});
</script>

<div class="flex h-screen flex-col bg-gray-100">
	<!-- Header -->
	<div class="flex justify-between gap-4 bg-white p-4 shadow-sm">
		<div class="flex items-center gap-4">
			<h1 class="text-xl font-bold">Canvas</h1>
			<div class="h-3 w-3 rounded-full {isConnected ? 'bg-green-500' : 'bg-red-500'}"></div>
			<span class="text-sm {isConnected ? 'text-green-600' : 'text-red-600'}">
				{isConnected ? 'Connected' : 'Disconnected'}
			</span>

			<!-- Stats Display -->
			{#if isConnected}
				<div class="flex items-center gap-4 border-l pl-4 text-sm text-gray-600">
					<span class="flex items-center gap-1">
						<People class="h-4 w-4" />
						<strong>Clients:</strong>
						{clientCount}
					</span>
					<span class="flex items-center gap-1">
						<Clock class="h-4 w-4" />
						<strong>RPS:</strong>
						{requestsPerSecond.toFixed(2)}
					</span>
				</div>
			{/if}
		</div>

		<div class="flex items-center gap-2">
			<label for="color-picker" class="text-sm">Color:</label>
			<input
				id="color-picker"
				type="color"
				bind:value={selectedColor}
				class="h-8 w-8 cursor-pointer rounded"
			/>

			<span class="text-sm">Zoom: {Math.round(scale * 100)}%</span>
			<button
				onclick={resetView}
				class="rounded bg-blue-500 px-3 py-1 text-sm text-white hover:bg-blue-600"
			>
				Reset View
			</button>
			<button
				onclick={centerView}
				class="rounded bg-green-500 px-3 py-1 text-sm text-white hover:bg-green-600"
			>
				Center
			</button>
		</div>
	</div>

	<!-- Canvas Container -->
	<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
	<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
	<div
		bind:this={canvasContainer}
		onwheel={handleWheel}
		onmousedown={handleMouseDown}
		onmousemove={handleMouseMove}
		onmouseup={handleMouseUp}
		class="relative flex-1 overflow-hidden bg-gray-200"
		role="application"
		tabindex="0"
		aria-label="Canvas background"
	>
		<canvas
			bind:this={canvas}
			class="absolute hidden cursor-crosshair border border-gray-300 bg-white"
			style="transform-origin: 0 0; image-rendering: pixelated;"
			aria-label="Canvas"
		></canvas>
	</div>

	<!-- Instructions -->
	<div class="border-t bg-white p-4 text-sm text-gray-600">
		<div class="flex items-center justify-center gap-6">
			<span><strong>Left Click + Drag:</strong> Paint pixels</span>
			<span><strong>Right Click + Drag:</strong> Pan canvas</span>
			<span><strong>Mouse Wheel:</strong> Zoom in/out</span>
		</div>
	</div>
</div>

<style>
	canvas {
		image-rendering: pixelated;
		image-rendering: -moz-crisp-edges;
		image-rendering: crisp-edges;
	}
</style>
