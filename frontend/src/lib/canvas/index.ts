import { CANVAS_WIDTH, CANVAS_HEIGHT, INITIAL_SCALE } from '$lib/constants';

export function setPixelLocal(
	ctx: CanvasRenderingContext2D,
	pixels: Uint32Array,
	x: number,
	y: number,
	color: number
) {
	if (x >= 0 && x <= CANVAS_WIDTH && y >= 0 && y <= CANVAS_HEIGHT) {
		pixels[y * CANVAS_WIDTH + x] = color;
		renderPixel(ctx, x, y, color);
	}
}

export function getPixelColor(pixels: Uint32Array, x: number, y: number): number {
	if (x < 0 || x >= CANVAS_WIDTH || y < 0 || y >= CANVAS_HEIGHT) {
		return 0; // out of bounds
	}
	return pixels[y * CANVAS_WIDTH + x];
}

export function renderPixel(ctx: CanvasRenderingContext2D, x: number, y: number, color: number) {
	if (!ctx) return;

	const r = (color >> 16) & 0xff;
	const g = (color >> 8) & 0xff;
	const b = color & 0xff;

	ctx.fillStyle = `rgb(${r}, ${g}, ${b})`;
	ctx.fillRect(x, y, 1, 1);
}

export function renderCanvas(ctx: CanvasRenderingContext2D, pixels: Uint32Array) {
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

export function updateCanvasPosition(
	canvas: HTMLCanvasElement | null,
	scale: number,
	offsetX: number,
	offsetY: number
) {
	if (!canvas) return;

	canvas.style.transform = `translate(${offsetX}px, ${offsetY}px) scale(${scale})`;
}

export function resetView(canvas: HTMLCanvasElement | null, canvasContainer: HTMLDivElement): { offsetX: number; offsetY: number; scale: number } | void {
	const centerResult = centerView(canvas, canvasContainer, INITIAL_SCALE);
	if (centerResult) {
		updateCanvasPosition(canvas, INITIAL_SCALE, centerResult.offsetX, centerResult.offsetY);
		return {
			offsetX: centerResult.offsetX,
			offsetY: centerResult.offsetY,
			scale: INITIAL_SCALE
		};
	}
	return undefined;
}

export function centerView(
	canvas: HTMLCanvasElement | null,
	canvasContainer: HTMLDivElement,
	scale: number
): { offsetX: number; offsetY: number; scale: number } | void {
	if (!canvasContainer || !canvas) return;

	const containerRect = canvasContainer.getBoundingClientRect();
	const canvasWidth = CANVAS_WIDTH * scale;
	const canvasHeight = CANVAS_HEIGHT * scale;

	const offsetX = (containerRect.width - canvasWidth) / 2;
	const offsetY = (containerRect.height - canvasHeight) / 2;

	return {
		offsetX,
		offsetY,
		scale
	};
}
