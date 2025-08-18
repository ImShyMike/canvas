<script lang="ts">
	import { onMount } from 'svelte';

	import { getRandomHexColor, hexToRGB, rgbToHex } from '$lib/color';
	import { unpackCoordinates, unpackRGB } from '$lib/protocol';
	import { MessageType, ResponseType } from '$lib/protocol';
	import type { Socket } from '$lib/socket';
	import { connectWebSocket } from '$lib/socket';
	import type { Stats } from '$lib/stats';
	import { sendSetPixel } from '$lib/socket';
	import {
		WEBSOCKET_URL,
		FLAVORS,
		CANVAS_WIDTH,
		CANVAS_HEIGHT,
		INITIAL_SCALE,
		MIN_SCALE,
		MAX_SCALE
	} from '$lib/constants';
	import {
		updateCanvasPosition,
		getPixelColor,
		centerView,
		setPixelLocal,
		renderCanvas,
		resetView
	} from '$lib/canvas';

	import Clock from 'virtual:icons/ic/round-access-time-filled';
	import People from 'virtual:icons/ic/baseline-people';

	// state stuff
	let canvas: HTMLCanvasElement;
	let ctx: CanvasRenderingContext2D;
	let socket = $state<Socket>({
		isConnected: false,
		ws: null
	});
	let isPainting = $state(false);
	let selectedColor = $state('#000000');
	let selectedFlavor = $state('auto');
	let dropdownShown = $state(false);
	let mobileWarning = $state(false);

	// stats state
	let stats: Stats = $state({
		clientCount: 0,
		requestsPerSecond: 0.0
	});

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

	// === websocket stuff ===
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
					setPixelLocal(ctx, pixels, x, y, color);
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
					renderCanvas(ctx, pixels);

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
					stats.clientCount = clientCountView.getUint32(0, false);
					stats.requestsPerSecond = rpsView.getFloat32(0, false);

					console.log(
						`Stats - Clients: ${stats.clientCount}, RPS: ${stats.requestsPerSecond.toFixed(2)}`
					);
				}
				break;
		}
	}

	// === mouse handlers ===
	function getCanvasCoordinates(event: MouseEvent): [number, number] {
		const rect = canvas.getBoundingClientRect();
		const clientX = event.clientX - rect.left;
		const clientY = event.clientY - rect.top;

		// convert from screen coordinates to canvas coordinates
		const canvasX = Math.floor(clientX / scale);
		const canvasY = Math.floor(clientY / scale);

		return [canvasX, canvasY];
	}

	function handleMouseDown(event: MouseEvent) {
		if (event.button === 0) {
			// left
			if ((event.target as HTMLElement) !== canvas) return; // don't start painting if not on canvas
			isPainting = true;
			let [x, y] = getCanvasCoordinates(event);
			if (x >= 0 && x <= CANVAS_WIDTH && y >= 0 && y <= CANVAS_HEIGHT) {
				const color = hexToRGB(selectedColor);
				x -= 1;
				y -= 1;
				if (getPixelColor(pixels, x, y) === color) {
					return;
				} // don't send if color is the same
				sendSetPixel(socket, x, y, color);
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
			let [x, y] = getCanvasCoordinates(event);
			if (x >= 0 && x <= CANVAS_WIDTH && y >= 0 && y <= CANVAS_HEIGHT) {
				const color = hexToRGB(selectedColor);
				x -= 1;
				y -= 1;
				if (getPixelColor(pixels, x, y) === color) {
					return;
				} // don't send if color is the same
				sendSetPixel(socket, x, y, color);
			}
		} else if (isDragging && (event.buttons === 2 || event.buttons === 4)) {
			// right/middle
			const deltaX = event.clientX - lastMouseX;
			const deltaY = event.clientY - lastMouseY;

			offsetX += deltaX;
			offsetY += deltaY;

			lastMouseX = event.clientX;
			lastMouseY = event.clientY;

			updateCanvasPosition(canvas, scale, offsetX, offsetY);
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

			updateCanvasPosition(canvas, scale, offsetX, offsetY);
		}
	}

	// === localstorage helpers ===
	function saveState() {
		if (!canvas || typeof window === 'undefined') return;

		const state = {
			scale,
			offsetX,
			offsetY,
			selectedColor,
			selectedFlavor
		};
		localStorage.setItem('settings', JSON.stringify(state));
	}

	function loadState() {
		if (typeof window === 'undefined') return;

		const state = localStorage.getItem('settings');
		if (state) {
			const {
				scale: savedScale,
				offsetX: savedOffsetX,
				offsetY: savedOffsetY,
				selectedColor: savedColor,
				selectedFlavor: savedFlavor
			} = JSON.parse(state);
			scale = savedScale || INITIAL_SCALE;
			offsetX = savedOffsetX || 0;
			offsetY = savedOffsetY || 0;
			selectedColor = savedColor || `#${getRandomHexColor()}`;
			selectedFlavor = savedFlavor || 'auto';
		}

		updateCanvasPosition(canvas, scale, offsetX, offsetY);
	}

	// === theme helpers ===
	function setTheme(flavor: string) {
		if (!flavor) return;

		// save the selected theme
		localStorage.setItem('theme', flavor);

		if (flavor === 'auto') {
			// for auto theme, listen to system preference changes
			const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
			const updateAutoTheme = () => {
				const autoTheme = mediaQuery.matches ? 'mocha' : 'latte';
				document.documentElement.className = autoTheme;
				console.log(`Auto theme set to: ${autoTheme}`);
			};

			// set initial auto theme
			updateAutoTheme();

			// listen for changes
			mediaQuery.removeEventListener('change', updateAutoTheme);
			mediaQuery.addEventListener('change', updateAutoTheme);

			selectedFlavor = 'auto';
		} else {
			// remove any existing media query listeners
			const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
			mediaQuery.removeEventListener('change', () => {});

			selectedFlavor = flavor;
			document.documentElement.className = selectedFlavor;
			console.log(`Setting theme to: ${selectedFlavor}`);
		}
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
		({ offsetX, offsetY, scale } = centerView(canvas, canvasContainer, scale) || {
			offsetX,
			offsetY,
			scale
		});

		// connect to the websocket
		connectWebSocket(socket, WEBSOCKET_URL, handleMessage);

		// prevent context menu opening on right click
		canvas.addEventListener('contextmenu', (e) => e.preventDefault());

		if (typeof window !== 'undefined') {
			// save state on unload (page close)
			window.addEventListener('beforeunload', () => {
				saveState();

				// close the websocket
				if (socket.ws) {
					socket.ws.close();
					socket.isConnected = false;
				}
			});

			// mobile warning
			window.addEventListener('resize', () => {
				if (window.innerWidth <= 768) {
					mobileWarning = true;
				} else {
					mobileWarning = false;
				}
			});
			if (window.innerWidth <= 768) {
				mobileWarning = true;
			}
		}

		// load saved state
		loadState();

		// set the initial theme
		setTheme(selectedFlavor);
	});

	$effect(() => {
		if (!socket.isConnected) {
			// unhide the canvas when disconnected to avoid a blank screen
			if (canvas) {
				canvas.style.display = 'block';
			}
		}
	});
</script>

<div class="flex h-screen flex-col overflow-hidden bg-base">
	<!-- Top Bar -->
	<div class="flex justify-between gap-4 bg-mantle p-4 shadow-sm">
		<div class="flex items-center gap-3">
			<h1 class="text-xl font-bold text-text">Canvas</h1>
			<div class="h-3 w-3 rounded-full {socket.isConnected ? 'bg-green' : 'bg-red'}"></div>
			<span class="text-sm {socket.isConnected ? 'text-green' : 'text-red'}">
				{socket.isConnected ? 'Connected' : 'Disconnected'}
			</span>

			<div class="flex items-center gap-4">
				{#if socket.isConnected}
					<div class="flex items-center gap-4 border-l border-overlay0 pl-4 text-sm text-subtext1">
						<span class="flex items-center gap-1">
							<People class="h-4 w-4" />
							<strong>Clients:</strong>
							{stats.clientCount}
						</span>
						<span class="flex items-center gap-1">
							<Clock class="h-4 w-4" />
							<strong>RPS:</strong>
							{stats.requestsPerSecond.toFixed(2)}
						</span>
					</div>
				{/if}
			</div>
		</div>

		<div class="flex items-center gap-2">
			<label for="color-picker" class="text-sm text-text">Color:</label>
			<input
				id="color-picker"
				type="color"
				bind:value={selectedColor}
				class="h-8 w-8 cursor-pointer rounded border border-overlay0"
			/>

			<span class="text-sm text-text">Zoom: {Math.round(scale * 100)}%</span>
			<button
				onclick={() => resetView(canvas, canvasContainer)}
				class="rounded border border-base bg-blue px-3 py-1 text-sm text-base hover:bg-sapphire"
			>
				Reset View
			</button>
			<button
				onclick={() =>
					({ offsetX, offsetY, scale } = centerView(canvas, canvasContainer, scale) || {
						offsetX,
						offsetY,
						scale
					})}
				class="rounded border border-base bg-green px-3 py-1 text-sm text-base hover:bg-teal"
			>
				Center
			</button>
			<div class="relative">
				<button
					class="flex items-center gap-2 rounded border border-overlay0 bg-surface0 px-3 py-1 text-sm text-text hover:bg-surface1"
					onclick={() => (dropdownShown = !dropdownShown)}
					title="Select Flavor"
				>
					<span class="text-sm">{FLAVORS.find((f) => f.id === selectedFlavor)?.emoji}</span>
					<span>{FLAVORS.find((f) => f.id === selectedFlavor)?.name}</span>
				</button>
				{#if dropdownShown}
					<div
						class="absolute right-0 z-10 mt-2 w-40 rounded border border-overlay0 bg-mantle shadow-lg"
					>
						{#each FLAVORS as flavor (flavor.id)}
							<button
								onclick={() => {
									setTheme(flavor.id);
									dropdownShown = false;
								}}
								class="flex w-full items-center gap-2 px-3 py-2 text-sm text-text hover:bg-surface0 {selectedFlavor ===
								flavor.id
									? 'bg-surface1'
									: ''}"
								title={flavor.name}
							>
								<span class="text-sm">{flavor.emoji}</span>
								<span>{flavor.name}</span>
							</button>
						{/each}
					</div>
				{/if}
			</div>
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
		class="relative flex-1 overflow-hidden bg-surface0"
		role="application"
		tabindex="0"
		aria-label="Canvas background"
	>
		<canvas
			bind:this={canvas}
			class="absolute hidden cursor-crosshair border border-overlay0 bg-base"
			style="transform-origin: 0 0;"
			aria-label="Canvas"
		></canvas>
	</div>

	<!-- Instructions -->
	<div class="border-t border-overlay0 bg-mantle p-4 text-sm text-subtext1">
		<div class="flex items-center justify-center gap-6">
			<span><strong class="text-text">Left Click + Drag:</strong> Paint pixels</span>
			<span><strong class="text-text">Right Click + Drag:</strong> Pan canvas</span>
			<span><strong class="text-text">Mouse Wheel:</strong> Zoom in/out</span>
		</div>
	</div>
</div>

<!-- Mobile Warning -->
{#if mobileWarning}
	<div
		id="warning"
		class="fixed inset-0 z-50 flex items-center justify-center bg-yellow text-mantle"
		style="position: fixed; top: 0; left: 0; right: 0; bottom: 0; width: 100vw; height: 100vh; margin: 0; padding: 0; overflow: hidden;"
	>
		<div class="mx-auto max-w-sm px-6 py-8 text-center">
			<strong class="mb-4 block text-lg">Warning</strong>
			<span class="mb-2 block text-sm"
				>This website was made for desktop, the website may look bad on mobile.</span
			>
			<span class="mb-6 block text-sm">(will be fixed later)</span>
			<button
				onclick={() => {
					mobileWarning = false;
				}}
				class="hover:bg-opacity-80 rounded bg-red px-4 py-2 text-mantle transition-colors"
			>
				Close
			</button>
		</div>
	</div>
{/if}

<style>
	canvas {
		image-rendering: pixelated;
		image-rendering: -moz-crisp-edges;
		image-rendering: crisp-edges;
	}
</style>
